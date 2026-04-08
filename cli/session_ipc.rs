/// Session IPC client — sends session commands to the runtime session actor.
///
/// All session access goes through IPC to the session actor. No direct
/// function calls to harmonia_gateway::sessions from CLI/TUI code.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use harmonia_gateway::sessions::{Session, SessionEvent, SessionSummary};

/// Send a length-prefixed sexp to the runtime and read the reply.
fn ipc_rpc(sexp: &str) -> Result<String, String> {
    let socket = runtime_socket_path().map_err(|e| e.to_string())?;
    let mut stream = UnixStream::connect(&socket)
        .map_err(|e| format!("session ipc connect: {e}"))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| format!("set timeout: {e}"))?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| format!("set timeout: {e}"))?;

    let payload = sexp.as_bytes();
    let len = (payload.len() as u32).to_be_bytes();
    stream.write_all(&len).map_err(|e| format!("write: {e}"))?;
    stream.write_all(payload).map_err(|e| format!("write: {e}"))?;
    stream.flush().map_err(|e| format!("flush: {e}"))?;

    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read: {e}"))?;
    let reply_len = u32::from_be_bytes(len_buf) as usize;
    if reply_len > 10 * 1024 * 1024 {
        return Err("reply too large".to_string());
    }
    let mut reply_buf = vec![0u8; reply_len];
    stream
        .read_exact(&mut reply_buf)
        .map_err(|e| format!("read: {e}"))?;
    Ok(String::from_utf8_lossy(&reply_buf).to_string())
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Extract the JSON payload from an (:ok :json "...") sexp reply.
fn extract_json(reply: &str) -> Result<String, String> {
    if reply.starts_with("(:error") {
        return Err(reply.to_string());
    }
    // Find :json "..." and extract the quoted string
    harmonia_actor_protocol::extract_sexp_string(reply, ":json")
        .ok_or_else(|| format!("no :json in reply: {reply}"))
}

/// Create a new session via IPC to the session actor.
pub fn create(node_label: &str) -> Result<Session, String> {
    let sexp = format!(
        "(:component \"sessions\" :op \"session-create\" :node-label \"{}\")",
        esc(node_label)
    );
    let reply = ipc_rpc(&sexp)?;
    let json = extract_json(&reply)?;
    serde_json::from_str(&json).map_err(|e| format!("parse session: {e}"))
}

/// List sessions via IPC.
pub fn list(node_label: &str) -> Result<Vec<SessionSummary>, String> {
    let sexp = format!(
        "(:component \"sessions\" :op \"session-list\" :node-label \"{}\")",
        esc(node_label)
    );
    let reply = ipc_rpc(&sexp)?;
    let json = extract_json(&reply)?;
    serde_json::from_str(&json).map_err(|e| format!("parse list: {e}"))
}

/// Resume a session via IPC.
pub fn resume(session_id: &str) -> Result<Session, String> {
    let sexp = format!(
        "(:component \"sessions\" :op \"session-resume\" :session-id \"{}\")",
        esc(session_id)
    );
    let reply = ipc_rpc(&sexp)?;
    let json = extract_json(&reply)?;
    serde_json::from_str(&json).map_err(|e| format!("parse session: {e}"))
}

/// Get current session via IPC.
#[allow(dead_code)]
pub fn current() -> Result<Option<Session>, String> {
    let sexp = "(:component \"sessions\" :op \"session-current\")";
    let reply = ipc_rpc(sexp)?;
    let json = extract_json(&reply)?;
    if json == "null" {
        return Ok(None);
    }
    serde_json::from_str(&json)
        .map(Some)
        .map_err(|e| format!("parse session: {e}"))
}

/// Read events via IPC.
pub fn read_events(session_id: &str) -> Result<Vec<SessionEvent>, String> {
    let sexp = if session_id.is_empty() {
        "(:component \"sessions\" :op \"session-events\")".to_string()
    } else {
        format!(
            "(:component \"sessions\" :op \"session-events\" :session-id \"{}\")",
            esc(session_id)
        )
    };
    let reply = ipc_rpc(&sexp)?;
    let json = extract_json(&reply)?;
    serde_json::from_str(&json).map_err(|e| format!("parse events: {e}"))
}

/// Append an event via IPC.
pub fn append_event(actor: &str, kind: &str, text: &str) -> Result<(), String> {
    let sexp = format!(
        "(:component \"sessions\" :op \"session-append\" :actor \"{}\" :kind \"{}\" :text \"{}\")",
        esc(actor),
        esc(kind),
        esc(text)
    );
    let reply = ipc_rpc(&sexp)?;
    if reply.starts_with("(:error") {
        Err(reply)
    } else {
        Ok(())
    }
}

/// Resolve the runtime socket path (same logic as modules.rs).
fn runtime_socket_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("HARMONIA_RUNTIME_SOCKET") {
        return Ok(PathBuf::from(path));
    }
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    let data_dir = crate::paths::data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
    }
    let _ = harmonia_config_store::init_v2();
    let state_root = harmonia_config_store::get_config_or(
        "harmonia-runtime",
        "global",
        "state-root",
        &default,
    )
    .unwrap_or(default);
    let path = PathBuf::from(state_root).join("runtime.sock");
    if !path.exists() {
        return Err(format!(
            "runtime socket not found at {} \u{2014} is harmonia running?",
            path.display()
        )
        .into());
    }
    Ok(path)
}
