use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread;

#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};

pub struct TuiState {
    pub inbound_queue: VecDeque<String>,
    pub clients: Vec<Arc<Mutex<BufWriter<UnixStream>>>>,
    pub listener_running: bool,
    pub initialized: bool,
    pub socket_path: String,
}

use std::io::BufWriter;

static STATE: OnceLock<Arc<RwLock<TuiState>>> = OnceLock::new();

fn state() -> &'static Arc<RwLock<TuiState>> {
    STATE.get_or_init(|| {
        Arc::new(RwLock::new(TuiState {
            inbound_queue: VecDeque::new(),
            clients: Vec::new(),
            listener_running: false,
            initialized: false,
            socket_path: String::new(),
        }))
    })
}

fn resolve_socket_path() -> String {
    if harmonia_config_store::init_v2().is_ok() {
        if let Ok(Some(run_dir)) =
            harmonia_config_store::get_config("harmonia-cli", "global", "run-dir")
        {
            let run_dir = std::path::PathBuf::from(run_dir);
            let _ = std::fs::create_dir_all(&run_dir);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700));
            }
            return format!("{}/harmonia.sock", run_dir.to_string_lossy());
        }
    }

    // Use platform-standard runtime directory, not user data dir.
    // macOS: $TMPDIR/harmonia/    Linux: $XDG_RUNTIME_DIR/harmonia/
    let run_dir = platform_run_dir();
    let _ = std::fs::create_dir_all(&run_dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&run_dir, std::fs::Permissions::from_mode(0o700));
    }
    format!("{}/harmonia.sock", run_dir.to_string_lossy())
}

#[cfg(target_os = "macos")]
fn platform_run_dir() -> std::path::PathBuf {
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        std::path::PathBuf::from(tmpdir).join("harmonia")
    } else {
        std::path::PathBuf::from("/tmp/harmonia")
    }
}

#[cfg(target_os = "linux")]
fn platform_run_dir() -> std::path::PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        std::path::PathBuf::from(xdg).join("harmonia")
    } else {
        let uid = unsafe { libc::getuid() };
        std::path::PathBuf::from(format!("/tmp/harmonia-{}", uid))
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_run_dir() -> std::path::PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("run").join("harmonia")
    } else {
        std::path::PathBuf::from("/tmp/harmonia")
    }
}

/// Initialize the TUI frontend.
/// Listens on a Unix domain socket for session client connections.
pub fn init() -> Result<(), String> {
    let st = state();
    {
        let guard = st.read().map_err(|e| format!("lock poisoned: {e}"))?;
        if guard.initialized {
            return Err("tui already initialized".into());
        }
    }

    let socket_path = resolve_socket_path();

    // Remove stale socket file
    let _ = std::fs::remove_file(&socket_path);

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&socket_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener =
        UnixListener::bind(&socket_path).map_err(|e| format!("bind {socket_path}: {e}"))?;

    // Set socket permissions (owner-only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600));
    }

    {
        let mut guard = st.write().map_err(|e| format!("lock poisoned: {e}"))?;
        guard.initialized = true;
        guard.listener_running = true;
        guard.socket_path = socket_path.clone();
    }

    // Accept connections in background thread
    let handle = Arc::clone(st);
    thread::spawn(move || {
        for stream in listener.incoming() {
            // Check if we should stop
            {
                let guard = match handle.read() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if !guard.listener_running {
                    break;
                }
            }
            match stream {
                Ok(stream) => {
                    let reader_stream = match stream.try_clone() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let writer = Arc::new(Mutex::new(BufWriter::new(stream)));

                    // Register client
                    if let Ok(mut guard) = handle.write() {
                        guard.clients.push(Arc::clone(&writer));
                    }

                    // Spawn reader thread for this client
                    let queue_handle = Arc::clone(&handle);
                    let writer_for_cleanup = Arc::clone(&writer);
                    thread::spawn(move || {
                        let reader = BufReader::new(reader_stream);
                        for line_result in reader.lines() {
                            match line_result {
                                Ok(line) => {
                                    if let Ok(mut guard) = queue_handle.write() {
                                        guard.inbound_queue.push_back(line);
                                    }
                                }
                                Err(_) => break, // client disconnected
                            }
                        }
                        // Remove client on disconnect
                        if let Ok(mut guard) = queue_handle.write() {
                            guard
                                .clients
                                .retain(|c| !Arc::ptr_eq(c, &writer_for_cleanup));
                        }
                    });
                }
                Err(_) => break,
            }
        }
    });

    eprintln!("[INFO] [tui] Listening on {socket_path}");
    Ok(())
}

/// Drain the inbound queue, returning (sub_channel, payload) pairs.
pub fn poll() -> Vec<(String, String)> {
    let st = state();
    let mut guard = match st.write() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    let mut results = Vec::with_capacity(guard.inbound_queue.len());
    while let Some(line) = guard.inbound_queue.pop_front() {
        results.push(("local".to_string(), line));
    }
    results
}

/// Send a message to all connected session clients.
pub fn send(sub_channel: &str, payload: &str) {
    let _ = sub_channel;
    let st = state();
    let guard = match st.read() {
        Ok(g) => g,
        Err(_) => return,
    };
    let msg = format!("{payload}\n");
    for client in &guard.clients {
        if let Ok(mut writer) = client.lock() {
            let _ = writer.write_all(msg.as_bytes());
            let _ = writer.flush();
        }
    }
}

/// Shut down the TUI listener and disconnect all clients.
pub fn shutdown() {
    let st = state();
    if let Ok(mut guard) = st.write() {
        guard.listener_running = false;
        guard.initialized = false;
        guard.clients.clear();
        // Remove socket file
        if !guard.socket_path.is_empty() {
            let _ = std::fs::remove_file(&guard.socket_path);
        }
    }
}
