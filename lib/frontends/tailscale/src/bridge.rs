use std::sync::{OnceLock, RwLock};

use harmonia_tailnet::model::{MeshMessage, MeshMessageType};
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
/// (node_id, payload) pairs.
///
/// Only messages of type `Signal` are returned; heartbeats, discovery, and
/// command messages are filtered out at the bridge level.
pub fn poll() -> Result<Vec<(String, String)>, String> {
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
            results.push((msg.from.clone(), msg.payload.clone()));
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

    let local_info = mesh::local_node_info().unwrap_or_else(|_| "unknown".into());

    let msg = MeshMessage {
        from: local_info,
        to: node_id.to_string(),
        payload: payload.to_string(),
        msg_type: MeshMessageType::Signal,
        timestamp_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        hmac: String::new(),
    };

    transport::send_message(node_id, &msg)
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
