//! Unix-domain-socket TUI server.
//!
//! [`TuiServer`] owns the listener and the live client list — there are no
//! module-level statics. Inbound lines arrive via a `std::sync::mpsc`
//! channel from a background acceptor thread; outbound writes fan out to
//! every connected client. When the actor is shut down, the socket file is
//! removed and the listener thread exits the next time `accept()` returns.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

use async_trait::async_trait;
use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

/// One connected operator client.
struct Client {
    writer: Mutex<BufWriter<UnixStream>>,
}

pub struct TuiServer {
    socket_path: String,
    inbound_rx: Mutex<mpsc::Receiver<String>>,
    clients: Arc<Mutex<Vec<Arc<Client>>>>,
    /// Shared with the acceptor thread; flipped to `false` on shutdown so the
    /// next accept loop iteration exits cleanly.
    running: Arc<AtomicBool>,
}

fn resolve_socket_path() -> String {
    if harmonia_config_store::init_v2().is_ok() {
        if let Ok(Some(run_dir)) =
            harmonia_config_store::get_config("harmonia-cli", "global", "run-dir")
        {
            let run_dir = PathBuf::from(run_dir);
            ensure_owner_dir(&run_dir);
            return format!("{}/harmonia.sock", run_dir.to_string_lossy());
        }
    }
    let run_dir = platform_run_dir();
    ensure_owner_dir(&run_dir);
    format!("{}/harmonia.sock", run_dir.to_string_lossy())
}

fn ensure_owner_dir(dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
}

#[cfg(target_os = "macos")]
fn platform_run_dir() -> PathBuf {
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        PathBuf::from(tmpdir).join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia")
    }
}

#[cfg(target_os = "linux")]
fn platform_run_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(xdg).join("harmonia")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/harmonia-{}", uid))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_run_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("run").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia")
    }
}

#[async_trait]
impl Frontend for TuiServer {
    type Config = ();

    fn name() -> &'static str {
        "tui"
    }

    fn security_label() -> &'static str {
        "owner"
    }

    async fn init(_: ()) -> Result<Self, String> {
        let socket_path = resolve_socket_path();
        let _ = std::fs::remove_file(&socket_path);
        if let Some(parent) = std::path::Path::new(&socket_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let listener = UnixListener::bind(&socket_path)
            .map_err(|e| format!("bind {socket_path}: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &socket_path,
                std::fs::Permissions::from_mode(0o600),
            );
        }

        let (tx, rx) = mpsc::channel::<String>();
        let clients: Arc<Mutex<Vec<Arc<Client>>>> = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        let clients_for_listener = Arc::clone(&clients);
        let running_for_listener = Arc::clone(&running);
        thread::spawn(move || acceptor_loop(listener, tx, clients_for_listener, running_for_listener));

        eprintln!("[INFO] [tui-server] listening on {socket_path}");
        Ok(Self {
            socket_path,
            inbound_rx: Mutex::new(rx),
            clients,
            running,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let mut out = VecDeque::new();
        // Lock briefly, drain whatever is queued, drop the lock.
        if let Ok(rx) = self.inbound_rx.lock() {
            while let Ok(line) = rx.try_recv() {
                out.push_back(InboundMessage {
                    address: "local".to_string(),
                    text: line,
                    metadata: None,
                });
            }
        }
        Ok(out.into_iter().collect())
    }

    async fn send(&mut self, _channel: &str, text: &str) -> Result<(), String> {
        let payload = format!("{text}\n");
        let snapshot: Vec<Arc<Client>> = match self.clients.lock() {
            Ok(g) => g.clone(),
            Err(_) => return Err("tui-server clients lock poisoned".into()),
        };
        for client in snapshot {
            if let Ok(mut w) = client.writer.lock() {
                let _ = w.write_all(payload.as_bytes());
                let _ = w.flush();
            }
        }
        Ok(())
    }

    async fn shutdown(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut g) = self.clients.lock() {
            g.clear();
        }
        if !self.socket_path.is_empty() {
            let _ = std::fs::remove_file(&self.socket_path);
        }
        // The acceptor thread is blocked on accept(); it will exit on its
        // next iteration when it observes `running == false`. We don't
        // forcibly join it — the OS reclaims the thread when the process
        // exits, and a graceful runtime shutdown gives it a moment via the
        // existing shutdown_timeout_secs path.
    }
}

fn acceptor_loop(
    listener: UnixListener,
    tx: mpsc::Sender<String>,
    clients: Arc<Mutex<Vec<Arc<Client>>>>,
    running: Arc<AtomicBool>,
) {
    for stream in listener.incoming() {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match stream {
            Ok(stream) => {
                let reader_stream = match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let writer = BufWriter::new(stream);
                let client = Arc::new(Client {
                    writer: Mutex::new(writer),
                });
                if let Ok(mut g) = clients.lock() {
                    g.push(Arc::clone(&client));
                }
                let tx_for_reader = tx.clone();
                let clients_for_cleanup = Arc::clone(&clients);
                let client_for_cleanup = Arc::clone(&client);
                thread::spawn(move || {
                    let reader = BufReader::new(reader_stream);
                    for line_result in reader.lines() {
                        match line_result {
                            Ok(line) => {
                                if tx_for_reader.send(line).is_err() {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    if let Ok(mut g) = clients_for_cleanup.lock() {
                        g.retain(|c| !Arc::ptr_eq(c, &client_for_cleanup));
                    }
                });
            }
            Err(_) => break,
        }
    }
    eprintln!("[INFO] [tui-server] acceptor exiting");
}
