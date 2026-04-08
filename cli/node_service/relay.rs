use harmonia_tailnet::model::{MeshMessageType, MeshSession};
use harmonia_tailnet::transport;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use super::{bind_socket, send_local_payload};

pub(super) fn run_client_mode(
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

pub(super) fn run_agent_mode(
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
