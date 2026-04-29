// ─��� Bridge: socket connection and daemon communication ──────────────

#[cfg(unix)]
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::theme::*;

/// Wait until a session server is accepting connections (not merely `harmonia.sock` present).
#[cfg(unix)]
pub(crate) fn wait_for_unix_session_connect(
    socket_path: &Path,
    status_text: &str,
    timeout_error: &str,
) -> Result<UnixStream, Box<dyn std::error::Error>> {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;
    for attempt in 0..300 {
        match UnixStream::connect(socket_path) {
            Ok(s) => {
                eprint!("\r                                     \r");
                return Ok(s);
            }
            Err(_) => {
                if attempt == 299 {
                    break;
                }
                eprint!("\r  {} {}", spinner_chars[i % 10], status_text);
                let _ = std::io::stderr().flush();
                i += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }
    eprint!("\r                                     \r");
    Err(timeout_error.into())
}

/// Unwrap gateway `{"text": "..."}` wrappers. Plain text passes through.
pub(crate) fn try_unwrap_json_text(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                return t.to_string();
            }
        }
    }
    line.to_string()
}

#[cfg(unix)]
pub(crate) fn send_to_daemon(
    writer: &mut UnixStream,
    message: &str,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = writeln!(writer, "{}", message) {
        eprintln!("  {RED}✗{RESET} Connection lost: {}", e);
        running.store(false, Ordering::Relaxed);
        return Err(e.into());
    }
    let _ = writer.flush();
    waiting.store(true, Ordering::Release);
    Ok(())
}
