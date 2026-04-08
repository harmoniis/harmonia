use harmonia_tailnet::mesh;
use harmonia_tailnet::model::{MeshMessageType, MeshSession};
use harmonia_tailnet::transport;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

struct PidGuard {
    path: PathBuf,
}

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn write_pid_file() -> Result<PidGuard, Box<dyn std::error::Error>> {
    let path = crate::paths::node_service_pid_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, std::process::id().to_string())?;
    Ok(PidGuard { path })
}

fn bind_socket(socket_path: &Path) -> Result<UnixListener, Box<dyn std::error::Error>> {
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let _ = fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(socket_path, fs::Permissions::from_mode(0o600));
    }
    listener.set_nonblocking(true)?;
    Ok(listener)
}

fn send_local_payload(
    writer: &mut BufWriter<UnixStream>,
    payload: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    for line in payload.lines() {
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn run_client_mode(
    node: crate::paths::NodeIdentity,
    session: harmonia_gateway::sessions::Session,
    pairing: crate::pairing::PairingRecord,
    socket_path: PathBuf,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = bind_socket(&socket_path)?;
    let (tx, rx) = mpsc::channel::<String>();
    let mut client_writer: Option<BufWriter<UnixStream>> = None;

    while running.load(Ordering::Relaxed) {
        if client_writer.is_none() {
            match listener.accept() {
                Ok((stream, _)) => {
                    let reader_stream = stream.try_clone()?;
                    let tx_reader = tx.clone();
                    let running_reader = Arc::clone(&running);
                    thread::spawn(move || {
                        let reader = BufReader::new(reader_stream);
                        for line in reader.lines() {
                            if !running_reader.load(Ordering::Relaxed) {
                                break;
                            }
                            match line {
                                Ok(input) => {
                                    if tx_reader.send(input).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                    client_writer = Some(BufWriter::new(stream));
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(err) => return Err(err.into()),
            }
        }

        while let Ok(input) = rx.try_recv() {
            let message = crate::node_rpc::outbound_message(
                &node,
                &pairing,
                MeshMessageType::Signal,
                input,
                Some(MeshSession {
                    id: session.id.clone(),
                    label: Some(session.id.clone()),
                }),
            );
            if let Err(err) = transport::send_message(&pairing.remote_addr, &message) {
                if let Some(writer) = client_writer.as_mut() {
                    let _ = send_local_payload(
                        writer,
                        &format!("[node-service] send remote session payload failed: {err}"),
                    );
                }
            }
        }

        for msg in transport::poll_messages() {
            if !crate::node_rpc::message_from_pairing(&pairing, &msg) {
                continue;
            }
            match msg.msg_type {
                MeshMessageType::Signal => {
                    if let Some(writer) = client_writer.as_mut() {
                        send_local_payload(writer, &msg.payload)?;
                    }
                }
                MeshMessageType::Command => {
                    if let Some(response) =
                        crate::node_rpc::handle_command_message(&node, &pairing, &msg)
                    {
                        if let Err(err) = transport::send_message(&pairing.remote_addr, &response) {
                            eprintln!("node-service rpc response send failed: {err}");
                        }
                    }
                }
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(50));
    }

    transport::stop_listener();
    let _ = fs::remove_file(socket_path);
    Ok(())
}

fn run_agent_mode(
    node: crate::paths::NodeIdentity,
    mut pairing: Option<crate::pairing::PairingRecord>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    while running.load(Ordering::Relaxed) {
        for msg in transport::poll_messages() {
            if pairing.is_none() {
                let learned = crate::pairing::pairing_from_mesh_message(&node, &msg);
                crate::pairing::save_default_pairing(&node, &learned)?;
                pairing = Some(learned);
            }
            let Some(active_pairing) = pairing.as_ref() else {
                continue;
            };
            if !crate::node_rpc::message_from_pairing(active_pairing, &msg) {
                continue;
            }
            match msg.msg_type {
                MeshMessageType::Signal => {
                    let response_lines =
                        match crate::node_rpc::relay_signal_to_local_agent(&msg.payload) {
                            Ok(lines) => lines,
                            Err(err) => vec![format!("[node-service] {err}")],
                        };
                    for line in response_lines {
                        let response = crate::node_rpc::outbound_message(
                            &node,
                            active_pairing,
                            MeshMessageType::Signal,
                            line,
                            msg.session.clone(),
                        );
                        if let Err(err) =
                            transport::send_message(&active_pairing.remote_addr, &response)
                        {
                            eprintln!("node-service agent relay response send failed: {err}");
                        }
                    }
                }
                MeshMessageType::Command => {
                    if let Some(response) =
                        crate::node_rpc::handle_command_message(&node, active_pairing, &msg)
                    {
                        if let Err(err) =
                            transport::send_message(&active_pairing.remote_addr, &response)
                        {
                            eprintln!("node-service rpc response send failed: {err}");
                        }
                    }
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    transport::stop_listener();
    Ok(())
}

pub fn run_foreground() -> Result<(), Box<dyn std::error::Error>> {
    let node = crate::paths::current_node_identity()?;
    let _pid_guard = write_pid_file()?;
    let _ = crate::node_link::load_or_create_identity(&node)?;
    mesh::init(&crate::node_rpc::mesh_service_config(&node))
        .map_err(|e| format!("tailnet mesh init failed: {e}"))?;
    transport::start_listener().map_err(|e| format!("tailnet listener failed: {e}"))?;

    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);
    let _ = ctrlc::set_handler(move || {
        running_ctrlc.store(false, Ordering::Relaxed);
    });

    match node.role {
        crate::paths::NodeRole::TuiClient => {
            let pairing = crate::pairing::ensure_pairing(&node)?;
            let data_dir = crate::paths::data_dir()?;
            let session = harmonia_gateway::sessions::create(&node.label, &data_dir)
                .map_err(|e| format!("session create: {e}"))?;
            run_client_mode(
                node,
                session,
                pairing,
                crate::paths::socket_path()?,
                running,
            )
        }
        crate::paths::NodeRole::Agent | crate::paths::NodeRole::MqttClient => run_agent_mode(
            node.clone(),
            crate::pairing::load_default_pairing(&node)?,
            running,
        ),
    }
}

pub fn ensure_background(
    node: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    let pid_path = crate::paths::node_service_pid_path()?;
    if pid_path.exists() {
        if let Ok(raw) = fs::read_to_string(&pid_path) {
            if let Ok(pid) = raw.trim().parse::<i32>() {
                #[cfg(unix)]
                if unsafe { libc::kill(pid, 0) } == 0 {
                    return Ok(());
                }
            }
        }
        let _ = fs::remove_file(&pid_path);
    }

    if node.role == crate::paths::NodeRole::TuiClient && crate::paths::socket_path()?.exists() {
        return Ok(());
    }

    let current_exe = std::env::current_exe()?;
    let log_path = crate::paths::node_service_log_path()?;
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err_file = log_file.try_clone()?;

    let wallet_db = crate::paths::wallet_db_path()?;
    let vault_db = crate::paths::vault_db_path()?;
    let state_root = crate::paths::data_dir()?;

    Command::new(current_exe)
        .arg("node-service")
        .env("HARMONIA_NODE_LABEL", &node.label)
        .env("HARMONIA_NODE_ROLE", node.role.as_str())
        .env("HARMONIA_INSTALL_PROFILE", node.install_profile.as_str())
        .env("HARMONIA_STATE_ROOT", state_root.to_string_lossy().as_ref())
        .env("HARMONIA_VAULT_DB", vault_db.to_string_lossy().as_ref())
        .env(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db.to_string_lossy().as_ref(),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()
        .map_err(|e| format!("spawn node-service failed: {e}"))?;

    Ok(())
}
