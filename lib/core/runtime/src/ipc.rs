use std::path::Path;

use ractor::ActorRef;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use harmonia_actor_protocol::ActorKind;

use crate::msg::RuntimeMsg;

/// Start the Unix domain socket IPC listener.
///
/// Accepts connections, reads length-prefixed sexp requests, dispatches
/// to the RuntimeSupervisor, and writes length-prefixed sexp replies.
pub async fn serve(socket_path: &str, supervisor: ActorRef<RuntimeMsg>) {
    // Remove stale socket file
    let _ = std::fs::remove_file(socket_path);

    // Ensure parent directory exists
    if let Some(parent) = Path::new(socket_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let listener = match UnixListener::bind(socket_path) {
        Ok(l) => {
            // Restrict socket to owner only (mode 0600)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600));
            }
            eprintln!("[INFO] [runtime] IPC listening on {socket_path}");
            l
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to bind IPC socket {socket_path}: {e}");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let sup = supervisor.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, sup).await {
                        eprintln!("[WARN] [runtime] IPC connection error: {e}");
                    }
                });
            }
            Err(e) => {
                eprintln!("[WARN] [runtime] IPC accept error: {e}");
            }
        }
    }
}

async fn handle_connection(
    mut stream: UnixStream,
    supervisor: ActorRef<RuntimeMsg>,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        // Read length-prefixed sexp: [4 bytes u32 BE length][sexp payload]
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(()); // Client disconnected
            }
            Err(e) => return Err(e.into()),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > 10 * 1024 * 1024 {
            return Err("message too large (>10MB)".into());
        }

        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;
        let sexp = String::from_utf8_lossy(&payload).to_string();

        // Dispatch and get reply
        let reply = dispatch_sexp(&sexp, &supervisor).await;

        // Write length-prefixed reply
        if let Some(reply_sexp) = reply {
            let reply_bytes = reply_sexp.as_bytes();
            let reply_len = (reply_bytes.len() as u32).to_be_bytes();
            stream.write_all(&reply_len).await?;
            stream.write_all(reply_bytes).await?;
        }
    }
}

/// Parse a sexp request and dispatch to the RuntimeSupervisor.
///
/// Returns Some(reply_sexp) for requests that expect a reply, None for fire-and-forget.
async fn dispatch_sexp(sexp: &str, supervisor: &ActorRef<RuntimeMsg>) -> Option<String> {
    let trimmed = sexp.trim();

    if trimmed.starts_with("(:drain") {
        let reply = ractor::call_t!(supervisor, RuntimeMsg::DrainSbcl, 10000);
        Some(reply.unwrap_or_else(|_| "()".to_string()))
    } else if trimmed.starts_with("(:register") {
        let kind_str = extract_string_value(trimmed, ":kind");
        let kind = kind_str
            .as_deref()
            .and_then(|s| ActorKind::from_str(s).ok())
            .unwrap_or(ActorKind::CliAgent);
        let reply = ractor::call_t!(supervisor, RuntimeMsg::Register, 5000, kind);
        match reply {
            Ok(id) => Some(format!("(:ok :id {})", id)),
            Err(_) => Some("(:error \"registration failed\")".to_string()),
        }
    } else if trimmed.starts_with("(:deregister") {
        let id = extract_u64_value(trimmed, ":id").unwrap_or(0);
        let reply = ractor::call_t!(supervisor, RuntimeMsg::Deregister, 5000, id);
        match reply {
            Ok(true) => Some("(:ok)".to_string()),
            Ok(false) => Some("(:error \"actor not found\")".to_string()),
            Err(_) => Some("(:error \"deregister failed\")".to_string()),
        }
    } else if trimmed.starts_with("(:heartbeat") {
        let id = extract_u64_value(trimmed, ":id").unwrap_or(0);
        let bytes_delta = extract_u64_value(trimmed, ":bytes-delta").unwrap_or(0);
        let _ = supervisor.cast(RuntimeMsg::Heartbeat { id, bytes_delta });
        None
    } else if trimmed.starts_with("(:post") {
        let source = extract_u64_value(trimmed, ":source").unwrap_or(0);
        let target = extract_u64_value(trimmed, ":target").unwrap_or(0);
        let payload_sexp = extract_string_value(trimmed, ":payload").unwrap_or_default();
        let _ = supervisor.cast(RuntimeMsg::Post {
            source,
            target,
            payload_sexp,
        });
        None
    } else if trimmed.starts_with("(:state") {
        let id = extract_u64_value(trimmed, ":id").unwrap_or(0);
        let reply = ractor::call_t!(supervisor, RuntimeMsg::GetState, 5000, id);
        Some(reply.unwrap_or_else(|_| "(:error \"state query failed\")".to_string()))
    } else if trimmed.starts_with("(:list") {
        let reply = ractor::call_t!(supervisor, RuntimeMsg::ListAll, 5000);
        Some(reply.unwrap_or_else(|_| "()".to_string()))
    } else if trimmed.starts_with("(:shutdown") {
        let _ = supervisor.cast(RuntimeMsg::Shutdown);
        Some("(:ok)".to_string())
    } else {
        Some(format!(
            "(:error \"unknown command: {}\")",
            harmonia_actor_protocol::sexp_escape(trimmed)
        ))
    }
}

// ── Minimal sexp value extractors ────────────────────────────────────

/// Maximum string value length to prevent DoS with huge payloads.
const MAX_STRING_VALUE_LEN: usize = 1024 * 1024; // 1 MB

fn extract_string_value(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = &sexp[idx + key.len()..];
    let after = after.trim_start();
    if after.starts_with('"') {
        let inner = &after[1..];
        let bytes = inner.as_bytes();
        if bytes.len() > MAX_STRING_VALUE_LEN {
            return None;
        }
        let mut end = 0;
        while end < bytes.len() {
            if bytes[end] == b'"' {
                return Some(inner[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
            }
            if bytes[end] == b'\\' {
                end += 1;
                // Bounds check: ensure the escaped char is within the string
                if end >= bytes.len() {
                    return None;
                }
            }
            end += 1;
        }
        None
    } else {
        let val: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ')')
            .collect();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }
}

fn extract_u64_value(sexp: &str, key: &str) -> Option<u64> {
    let idx = sexp.find(key)?;
    let after = &sexp[idx + key.len()..];
    let after = after.trim_start();
    let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    num_str.parse().ok()
}
