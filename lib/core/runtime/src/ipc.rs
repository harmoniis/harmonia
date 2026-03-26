use std::path::Path;

use ractor::ActorRef;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use harmonia_actor_protocol::ActorKind;

use crate::actors::ComponentMsg;
use crate::component_registry::SharedRegistry;
use crate::msg::RuntimeMsg;

/// Start the Unix domain socket IPC listener.
///
/// Component data calls dispatch DIRECTLY to actor mailboxes via the registry,
/// bypassing the supervisor entirely. Only lifecycle calls go through the supervisor.
pub async fn serve(socket_path: &str, supervisor: ActorRef<RuntimeMsg>, registry: SharedRegistry) {
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
                let reg = registry.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, sup, reg).await {
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
    registry: SharedRegistry,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(());
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

        let reply = dispatch_sexp(&sexp, &supervisor, &registry).await;

        // Write length-prefixed reply
        if let Some(reply_sexp) = reply {
            let reply_bytes = reply_sexp.as_bytes();
            let reply_len = (reply_bytes.len() as u32).to_be_bytes();
            stream.write_all(&reply_len).await?;
            stream.write_all(reply_bytes).await?;
        }
    }
}

/// Parse a sexp request and dispatch.
///
/// Component data calls go DIRECTLY to actor mailboxes via the registry.
/// Lifecycle calls (register, deregister, list) go through the supervisor.
/// No supervisor bottleneck on the hot path.
async fn dispatch_sexp(sexp: &str, supervisor: &ActorRef<RuntimeMsg>, registry: &SharedRegistry) -> Option<String> {
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
    } else if trimmed.starts_with("(:component") {
        // Component dispatch: (:component "vault" :op "set-secret" :symbol "x" :value "y")
        let component = extract_string_value(trimmed, ":component").unwrap_or_default();

        // Fast path: observability trace ops are fire-and-forget.
        // Cast directly to the obs actor, bypass supervisor entirely.
        if component == "observability" {
            let op = extract_string_value(trimmed, ":op").unwrap_or_default();
            if matches!(op.as_str(), "trace-start" | "trace-end" | "trace-event") {
                crate::dispatch::dispatch_obs_trace(&op, trimmed);
                return None;
            }
        }

        // DIRECT DISPATCH — bypass supervisor entirely for data calls.
        // The component actor processes the request in its own mailbox.
        // No supervisor bottleneck. Parallel calls don't block each other.
        if let Some(actor) = crate::component_registry::get(registry, &component) {
            // Dispatch directly to the component actor. No supervisor in the path.
            // Timeout is generous because LLM calls take 10-60s and the user
            // interrupts with ESC, not a forced timeout.
            match ractor::call_t!(actor, ComponentMsg::Dispatch, 120_000, trimmed.to_string()) {
                Ok(result) => Some(result),
                Err(_) => Some(format!("(:error \"component '{}' dispatch timeout\")", component)),
            }
        } else {
            // Component not in registry — fall back to supervisor (lifecycle ops).
            let comp_name = component.clone();
            let reply = ractor::call_t!(
                supervisor,
                RuntimeMsg::ComponentCall,
                10_000,
                component,
                trimmed.to_string()
            );
            Some(reply.unwrap_or_else(|_| format!("(:error \"unknown component '{}'\")", comp_name)))
        }
    } else if trimmed.starts_with("(:modules") {
        let op = extract_string_value(trimmed, ":op").unwrap_or_default();
        match op.as_str() {
            "list" => {
                let reply = ractor::call_t!(supervisor, RuntimeMsg::ListModules, 5000);
                Some(reply.unwrap_or_else(|_| "(:error \"list modules timeout\")".to_string()))
            }
            "load" => {
                let name = extract_string_value(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    let reply = ractor::call_t!(supervisor, RuntimeMsg::LoadModule, 10000, name);
                    Some(reply.unwrap_or_else(|_| "(:error \"load module timeout\")".to_string()))
                }
            }
            "unload" => {
                let name = extract_string_value(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    let reply = ractor::call_t!(supervisor, RuntimeMsg::UnloadModule, 10000, name);
                    Some(reply.unwrap_or_else(|_| "(:error \"unload module timeout\")".to_string()))
                }
            }
            "reload" => {
                let name = extract_string_value(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    // Unload first, then load
                    let _ =
                        ractor::call_t!(supervisor, RuntimeMsg::UnloadModule, 10000, name.clone());
                    let reply = ractor::call_t!(supervisor, RuntimeMsg::LoadModule, 10000, name);
                    Some(reply.unwrap_or_else(|_| "(:error \"reload module timeout\")".to_string()))
                }
            }
            _ => Some(format!(
                "(:error \"unknown modules op: {}\")",
                harmonia_actor_protocol::sexp_escape(&op)
            )),
        }
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
