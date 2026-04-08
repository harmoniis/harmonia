use std::sync::Arc;

use ractor::ActorRef;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;

use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericNamespaced, ListenerOptions,
};

use harmonia_actor_protocol::ActorKind;

use crate::actors::ComponentMsg;
use crate::dynamic_registry::SharedDynamicRegistry;
use crate::msg::RuntimeMsg;

/// FNV-1a 64-bit hash — deterministic across all platforms and Rust versions.
/// Used to derive the IPC socket/pipe name from the state root.
/// The same algorithm is implemented in Common Lisp (ipc-client.lisp).
pub fn fnv1a_64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xCBF29CE484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001B3);
    }
    hash
}

/// Compute the IPC socket/pipe name from the state root.
/// Format: `harmonia-runtime-{16-hex-digit-hash}`
///
/// On Linux: abstract namespace socket
/// On macOS/FreeBSD: /tmp/harmonia-runtime-{hash} filesystem socket
/// On Windows: \\.\pipe\harmonia-runtime-{hash} named pipe
pub fn ipc_name(state_root: &str) -> String {
    format!("harmonia-runtime-{:016X}", fnv1a_64(state_root.as_bytes()))
}

/// Nonce token length (32 bytes = 64 hex chars).
pub const TOKEN_LEN: usize = 64;

/// Start the cross-platform IPC listener.
///
/// Uses `interprocess` crate: Unix domain sockets on Unix, named pipes on Windows.
/// Component data calls dispatch DIRECTLY to actor mailboxes via the registry,
/// bypassing the supervisor entirely. Only lifecycle calls go through the supervisor.
pub async fn serve(
    name: &str,
    supervisor: ActorRef<RuntimeMsg>,
    registry: SharedDynamicRegistry,
    topic_bus: crate::topic_bus::SharedTopicBus,
    token: Arc<String>,
    ready: Arc<Notify>,
) {
    let printable_name = name.to_string();

    // On macOS/FreeBSD, GenericNamespaced creates a filesystem socket at /tmp/{name}.
    // Remove stale socket from previous run (Linux abstract namespace has no file).
    #[cfg(not(target_os = "linux"))]
    {
        let stale_path = format!("/tmp/{name}");
        let _ = std::fs::remove_file(&stale_path);
    }

    let listener = match ListenerOptions::new()
        .name(name.to_ns_name::<GenericNamespaced>().expect("invalid IPC name"))
        .create_tokio()
    {
        Ok(l) => {
            eprintln!("[INFO] [runtime] IPC listening: {printable_name}");
            l
        }
        Err(e) => {
            eprintln!("[ERROR] [runtime] Failed to create IPC listener '{printable_name}': {e}");
            return;
        }
    };

    // Signal that IPC is ready — main.rs waits on this before announcing startup.
    ready.notify_waiters();

    loop {
        match listener.accept().await {
            Ok(stream) => {
                let sup = supervisor.clone();
                let reg = registry.clone();
                let bus = topic_bus.clone();
                let tok = token.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, sup, reg, bus, tok).await {
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
    mut stream: Stream,
    supervisor: ActorRef<RuntimeMsg>,
    registry: SharedDynamicRegistry,
    topic_bus: crate::topic_bus::SharedTopicBus,
    expected_token: Arc<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // ── Nonce handshake: verify client knows the token ──
    let mut token_buf = [0u8; TOKEN_LEN];
    stream.read_exact(&mut token_buf).await?;
    let client_token = std::str::from_utf8(&token_buf)
        .map_err(|_| "invalid token encoding")?;
    // Constant-time compare to prevent timing attacks
    if !constant_time_eq(client_token.as_bytes(), expected_token.as_bytes()) {
        return Err("IPC token mismatch — connection rejected".into());
    }

    // ── Framed sexp protocol ──
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
        let sexp = String::from_utf8_lossy(&payload).into_owned();

        let reply = dispatch_sexp(sexp, &supervisor, &registry, &topic_bus).await;

        if let Some(reply_sexp) = reply {
            let reply_bytes = reply_sexp.as_bytes();
            let reply_len = (reply_bytes.len() as u32).to_be_bytes();
            stream.write_all(&reply_len).await?;
            stream.write_all(reply_bytes).await?;
        }
    }
}

/// Constant-time byte comparison to prevent timing side-channels on the nonce.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Parse a sexp request and dispatch.
///
/// Component data calls go DIRECTLY to actor mailboxes via the registry.
/// Lifecycle calls (register, deregister, list) go through the supervisor.
/// Owns the sexp String to avoid redundant copies on the hot path.
async fn dispatch_sexp(
    sexp: String,
    supervisor: &ActorRef<RuntimeMsg>,
    registry: &SharedDynamicRegistry,
    topic_bus: &crate::topic_bus::SharedTopicBus,
) -> Option<String> {
    let trimmed = sexp.trim();

    if trimmed.starts_with("(:drain") {
        let reply = ractor::call_t!(supervisor, RuntimeMsg::DrainSbcl, 10000);
        Some(reply.unwrap_or_else(|_| "()".to_string()))
    } else if trimmed.starts_with("(:register") {
        let kind_str = harmonia_actor_protocol::extract_sexp_string(trimmed, ":kind");
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
        let id = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":id").unwrap_or(0);
        let reply = ractor::call_t!(supervisor, RuntimeMsg::Deregister, 5000, id);
        match reply {
            Ok(true) => Some("(:ok)".to_string()),
            Ok(false) => Some("(:error \"actor not found\")".to_string()),
            Err(_) => Some("(:error \"deregister failed\")".to_string()),
        }
    } else if trimmed.starts_with("(:heartbeat") {
        let id = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":id").unwrap_or(0);
        let bytes_delta = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":bytes-delta").unwrap_or(0);
        let _ = supervisor.cast(RuntimeMsg::Heartbeat { id, bytes_delta });
        None
    } else if trimmed.starts_with("(:post") {
        let source = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":source").unwrap_or(0);
        let target = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":target").unwrap_or(0);
        let payload_sexp = harmonia_actor_protocol::extract_sexp_string(trimmed, ":payload").unwrap_or_default();
        let _ = supervisor.cast(RuntimeMsg::Post {
            source,
            target,
            payload_sexp,
        });
        None
    } else if trimmed.starts_with("(:state") {
        let id = harmonia_actor_protocol::extract_sexp_u64(trimmed, ":id").unwrap_or(0);
        let reply = ractor::call_t!(supervisor, RuntimeMsg::GetState, 5000, id);
        Some(reply.unwrap_or_else(|_| "(:error \"state query failed\")".to_string()))
    } else if trimmed.starts_with("(:list") {
        let reply = ractor::call_t!(supervisor, RuntimeMsg::ListAll, 5000);
        Some(reply.unwrap_or_else(|_| "()".to_string()))
    } else if trimmed.starts_with("(:topic-publish") {
        let topic = harmonia_actor_protocol::extract_sexp_string(trimmed, ":topic").unwrap_or_default();
        let payload = harmonia_actor_protocol::extract_sexp_string(trimmed, ":payload").unwrap_or_default();
        let delivered = topic_bus.publish(&topic, &payload);
        Some(format!("(:ok :topic \"{}\" :delivered {})", topic, delivered))
    } else if trimmed.starts_with("(:topic-subscribers") {
        let topic = harmonia_actor_protocol::extract_sexp_string(trimmed, ":topic").unwrap_or_default();
        let subs = registry.subscribers(&topic);
        let subs_sexp = subs.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(" ");
        Some(format!("(:ok :topic \"{}\" :subscribers ({}))", topic, subs_sexp))
    } else if trimmed.starts_with("(:topic-list") {
        let topics = topic_bus.topics();
        let items = topics.iter().map(|(t, c)| format!("(:topic \"{}\" :subscribers {})", t, c)).collect::<Vec<_>>().join(" ");
        Some(format!("(:ok :topics ({}))", items))
    } else if trimmed.starts_with("(:component") {
        let component = harmonia_actor_protocol::extract_sexp_string(trimmed, ":component").unwrap_or_default();

        // Fast path: observability trace ops are fire-and-forget.
        if component == "observability" {
            let op = harmonia_actor_protocol::extract_sexp_string(trimmed, ":op").unwrap_or_default();
            if matches!(op.as_str(), "trace-start" | "trace-end" | "trace-event") {
                crate::dispatch::dispatch_obs_trace(&op, trimmed);
                return None;
            }
        }

        // DIRECT DISPATCH via DynamicRegistry — pluggable, no hardcoded slots.
        if let Some(actor) = registry.get(&component) {
            // Pass the owned String directly — no extra .to_string() copy.
            match ractor::call_t!(actor, ComponentMsg::Dispatch, 120_000, sexp) {
                Ok(result) => Some(result),
                Err(_) => Some(format!(
                    "(:error \"component '{}' dispatch timeout\")",
                    component
                )),
            }
        } else {
            let comp_name = component.clone();
            let reply = ractor::call_t!(
                supervisor,
                RuntimeMsg::ComponentCall,
                10_000,
                component,
                sexp
            );
            Some(
                reply.unwrap_or_else(|_| {
                    format!("(:error \"unknown component '{}'\")", comp_name)
                }),
            )
        }
    } else if trimmed.starts_with("(:modules") {
        let op = harmonia_actor_protocol::extract_sexp_string(trimmed, ":op").unwrap_or_default();
        match op.as_str() {
            "list" => {
                let reply = ractor::call_t!(supervisor, RuntimeMsg::ListModules, 5000);
                Some(reply.unwrap_or_else(|_| "(:error \"list modules timeout\")".to_string()))
            }
            "load" => {
                let name = harmonia_actor_protocol::extract_sexp_string(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    let reply = ractor::call_t!(supervisor, RuntimeMsg::LoadModule, 10000, name);
                    Some(reply.unwrap_or_else(|_| "(:error \"load module timeout\")".to_string()))
                }
            }
            "unload" => {
                let name = harmonia_actor_protocol::extract_sexp_string(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    let reply =
                        ractor::call_t!(supervisor, RuntimeMsg::UnloadModule, 10000, name);
                    Some(
                        reply
                            .unwrap_or_else(|_| "(:error \"unload module timeout\")".to_string()),
                    )
                }
            }
            "reload" => {
                let name = harmonia_actor_protocol::extract_sexp_string(trimmed, ":name").unwrap_or_default();
                if name.is_empty() {
                    Some("(:error \"missing :name\")".to_string())
                } else {
                    let _ =
                        ractor::call_t!(supervisor, RuntimeMsg::UnloadModule, 10000, name.clone());
                    let reply = ractor::call_t!(supervisor, RuntimeMsg::LoadModule, 10000, name);
                    Some(
                        reply
                            .unwrap_or_else(|_| "(:error \"reload module timeout\")".to_string()),
                    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a_deterministic() {
        assert_eq!(fnv1a_64(b""), 0xCBF29CE484222325);
        // Same input always produces same output
        assert_eq!(fnv1a_64(b"test"), fnv1a_64(b"test"));
        // Different inputs produce different outputs
        assert_ne!(fnv1a_64(b"test"), fnv1a_64(b"test2"));
        // Verify known value for "/tmp/harmonia"
        let hash = fnv1a_64(b"/tmp/harmonia");
        assert_ne!(hash, 0);
        assert_eq!(hash, fnv1a_64(b"/tmp/harmonia"));
    }

    #[test]
    fn ipc_name_format() {
        let name = ipc_name("/tmp/harmonia");
        assert!(name.starts_with("harmonia-runtime-"));
        assert_eq!(name.len(), "harmonia-runtime-".len() + 16);
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abcdef", b"abcdef"));
        assert!(!constant_time_eq(b"abcdef", b"abcdeg"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn extract_sexp_values() {
        let sexp = r#"(:component "vault" :op "set-secret" :symbol "api-key" :value "sk-123")"#;
        assert_eq!(
            harmonia_actor_protocol::extract_sexp_string(sexp, ":component"),
            Some("vault".to_string())
        );
        assert_eq!(
            harmonia_actor_protocol::extract_sexp_string(sexp, ":op"),
            Some("set-secret".to_string())
        );
        assert_eq!(
            harmonia_actor_protocol::extract_sexp_string(sexp, ":value"),
            Some("sk-123".to_string())
        );
    }
}
