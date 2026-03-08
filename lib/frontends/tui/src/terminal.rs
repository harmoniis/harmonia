use std::collections::VecDeque;
use std::io::{self, BufRead};
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;

pub struct TuiState {
    pub inbound_queue: VecDeque<String>,
    pub reader_running: bool,
    pub initialized: bool,
}

static STATE: OnceLock<Arc<RwLock<TuiState>>> = OnceLock::new();

fn state() -> &'static Arc<RwLock<TuiState>> {
    STATE.get_or_init(|| {
        Arc::new(RwLock::new(TuiState {
            inbound_queue: VecDeque::new(),
            reader_running: false,
            initialized: false,
        }))
    })
}

/// Initialize the TUI frontend.
/// Spawns a background thread that reads lines from stdin and pushes them
/// into the inbound queue with sub_channel "local".
pub fn init() -> Result<(), String> {
    let st = state();
    {
        let guard = st.read().map_err(|e| format!("lock poisoned: {e}"))?;
        if guard.initialized {
            return Err("tui already initialized".into());
        }
    }

    {
        let mut guard = st.write().map_err(|e| format!("lock poisoned: {e}"))?;
        guard.initialized = true;
        guard.reader_running = true;
    }

    let handle = Arc::clone(st);
    thread::spawn(move || {
        let stdin = io::stdin();
        let reader = stdin.lock();
        for line_result in reader.lines() {
            // Check if we should stop
            {
                let guard = match handle.read() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if !guard.reader_running {
                    break;
                }
            }
            match line_result {
                Ok(line) => {
                    if let Ok(mut guard) = handle.write() {
                        guard.inbound_queue.push_back(line);
                    }
                }
                Err(_) => break,
            }
        }
    });

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

/// Send a message to the terminal: write payload to stdout with a prefix.
pub fn send(sub_channel: &str, payload: &str) {
    let _ = sub_channel; // sub_channel unused for TUI stdout; always writes to terminal
    println!("[harmonia] {payload}");
}

/// Shut down the TUI reader thread.
pub fn shutdown() {
    let st = state();
    if let Ok(mut guard) = st.write() {
        guard.reader_running = false;
        guard.initialized = false;
    }
}
