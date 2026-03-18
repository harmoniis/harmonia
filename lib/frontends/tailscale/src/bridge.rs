use std::sync::{OnceLock, RwLock};

use harmonia_tailnet::model::{MeshMessage, MeshMessageType, MeshOrigin};
use harmonia_tailnet::{mesh, transport};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct TailscaleBridgeState {
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<TailscaleBridgeState>> = OnceLock::new();

fn state() -> &'static RwLock<TailscaleBridgeState> {
    STATE.get_or_init(|| RwLock::new(TailscaleBridgeState { initialized: false }))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the tailscale frontend bridge.
///
/// Delegates to `harmonia_tailnet::mesh::init` for mesh configuration, then
/// starts the TCP transport listener via `transport::start_listener`.
///
/// Config is an s-expression string passed through to the tailnet crate.
pub fn init(config: &str) -> Result<(), String> {
    {
        let s = state().read().map_err(|e| format!("lock: {}", e))?;
        if s.initialized {
            return Err("tailscale frontend already initialized".into());
        }
    }

    mesh::init(config)?;
    transport::start_listener()?;

    {
        let mut s = state().write().map_err(|e| format!("lock: {}", e))?;
        s.initialized = true;
    }

    Ok(())
}

/// Poll the tailnet transport for incoming messages and return them as
/// (node_id, payload, metadata) triples.
///
/// Only messages of type `Signal` are returned; heartbeats, discovery, and
/// command messages are filtered out at the bridge level.
pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    {
        let s = state().read().map_err(|e| format!("lock: {}", e))?;
        if !s.initialized {
            return Err("tailscale frontend not initialized".into());
        }
    }

    let messages = transport::poll_messages();
    let mut results = Vec::new();

    for msg in messages {
        if msg.msg_type == MeshMessageType::Signal {
            let metadata = build_metadata(&msg);
            results.push((
                msg.origin
                    .as_ref()
                    .map(|origin| origin.node_id.clone())
                    .unwrap_or_else(|| msg.from.clone()),
                msg.payload.clone(),
                metadata,
            ));
        }
    }

    Ok(results)
}

/// Send a Signal message to a remote node.
///
/// `node_id` should include the port, e.g. "100.64.1.1:7483".
/// `payload` is the signal payload string.
pub fn send(node_id: &str, payload: &str) -> Result<(), String> {
    {
        let s = state().read().map_err(|e| format!("lock: {}", e))?;
        if !s.initialized {
            return Err("tailscale frontend not initialized".into());
        }
    }

    let local_node = mesh::local_node()?;
    let node_id_value = local_node.id.0.clone();
    let node_label_value = local_node.label.clone();
    let node_role_value = local_node.role.clone();

    let msg = MeshMessage {
        from: node_id_value.clone(),
        to: node_id.to_string(),
        payload: payload.to_string(),
        msg_type: MeshMessageType::Signal,
        origin: Some(MeshOrigin {
            node_id: node_id_value,
            node_label: Some(node_label_value),
            node_role: Some(node_role_value),
            channel_class: Some("tailscale-agent".to_string()),
            node_key_id: None,
            transport_security: None,
        }),
        session: None,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        hmac: String::new(),
    };

    transport::send_message(node_id, &msg)
}

fn build_metadata(msg: &MeshMessage) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(origin) = &msg.origin {
        parts.push(format!(":node-id \"{}\"", escape_metadata(&origin.node_id)));
        if let Some(label) = origin.node_label.as_deref() {
            parts.push(format!(":node-label \"{}\"", escape_metadata(label)));
        }
        if let Some(role) = origin.node_role.as_deref() {
            parts.push(format!(":node-role \"{}\"", escape_metadata(role)));
        }
        if let Some(channel_class) = origin.channel_class.as_deref() {
            parts.push(format!(
                ":channel-class \"{}\"",
                escape_metadata(channel_class)
            ));
        }
        if let Some(node_key_id) = origin.node_key_id.as_deref() {
            parts.push(format!(":node-key-id \"{}\"", escape_metadata(node_key_id)));
        }
        if let Some(transport_security) = origin.transport_security.as_deref() {
            parts.push(format!(
                ":transport-security \"{}\"",
                escape_metadata(transport_security)
            ));
        }
    } else if !msg.from.is_empty() {
        parts.push(format!(":node-id \"{}\"", escape_metadata(&msg.from)));
    }
    if let Some(session) = &msg.session {
        parts.push(format!(":session-id \"{}\"", escape_metadata(&session.id)));
        if let Some(label) = session.label.as_deref() {
            parts.push(format!(":session-label \"{}\"", escape_metadata(label)));
        }
    }
    if parts.is_empty() {
        None
    } else {
        parts.push(":remote t".to_string());
        Some(format!("({})", parts.join(" ")))
    }
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Shut down the tailscale frontend bridge.
///
/// Stops the transport listener and resets state.
pub fn shutdown() {
    transport::stop_listener();
    if let Ok(mut s) = state().write() {
        s.initialized = false;
    }
}
