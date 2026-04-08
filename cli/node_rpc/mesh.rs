//! Tailnet mesh helpers: addressing, message construction, remote RPC.

use harmonia_tailnet::mesh;
use harmonia_tailnet::model::{MeshMessage, MeshMessageType, MeshOrigin, MeshSession};
use harmonia_tailnet::transport;
use std::thread;
use std::time::{Duration, Instant};

use harmonia_node_rpc::{NodeRpcRequest, NodeRpcResponseEnvelope, RpcEnvelope};

use super::helpers::now_ms;

pub fn mesh_service_config(node: &crate::paths::NodeIdentity) -> String {
    format!(
        "(tailnet-config (id \"{}\") (label \"{}\") (role \"{}\") (port {}) (frontends \"tailscale\") (tools \"session\" \"node-rpc\" \"fs\" \"shell\" \"tmux\" \"wallet\"))",
        crate::pairing::advertised_addr(node),
        node.label,
        node.role.as_str(),
        crate::pairing::tailnet_port()
    )
}

pub fn channel_class_for_node(node: &crate::paths::NodeIdentity) -> &'static str {
    match node.role {
        crate::paths::NodeRole::Agent => "tailscale-agent",
        crate::paths::NodeRole::TuiClient | crate::paths::NodeRole::MqttClient => {
            "tailscale-client"
        }
    }
}

pub fn message_from_pairing(pairing: &crate::pairing::PairingRecord, msg: &MeshMessage) -> bool {
    if msg.from == pairing.remote_addr {
        return true;
    }
    if let Some(origin) = &msg.origin {
        if origin.node_id == pairing.remote_addr {
            return true;
        }
        if origin
            .node_label
            .as_deref()
            .map(|label| label == pairing.remote_label)
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

pub fn outbound_message(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    msg_type: MeshMessageType,
    payload: String,
    session: Option<MeshSession>,
) -> MeshMessage {
    let node_addr = crate::pairing::advertised_addr(node);
    MeshMessage {
        from: node_addr.clone(),
        to: pairing.remote_addr.clone(),
        payload,
        msg_type,
        origin: Some(MeshOrigin {
            node_id: node_addr,
            node_label: Some(node.label.clone()),
            node_role: Some(node.role.as_str().to_string()),
            channel_class: Some(channel_class_for_node(node).to_string()),
            node_key_id: None,
            transport_security: None,
        }),
        session,
        timestamp_ms: now_ms(),
        hmac: String::new(),
    }
}

pub fn request_remote(
    node: &crate::paths::NodeIdentity,
    pairing: &crate::pairing::PairingRecord,
    request: NodeRpcRequest,
    timeout_ms: u64,
) -> Result<NodeRpcResponseEnvelope, Box<dyn std::error::Error>> {
    mesh::init(&mesh_service_config(node)).map_err(|e| format!("tailnet mesh init failed: {e}"))?;
    transport::start_listener().map_err(|e| format!("tailnet listener failed: {e}"))?;

    let request_id = format!("rpc-{}", now_ms());
    let payload = serde_json::to_string(&RpcEnvelope::new(request_id.clone(), request))?;
    let message = outbound_message(node, pairing, MeshMessageType::Command, payload, None);
    transport::send_message(&pairing.remote_addr, &message)
        .map_err(|e| format!("send remote rpc failed: {e}"))?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(1));
    loop {
        for msg in transport::poll_messages() {
            if msg.msg_type != MeshMessageType::Command || !message_from_pairing(pairing, &msg) {
                continue;
            }
            let response: NodeRpcResponseEnvelope = match serde_json::from_str(&msg.payload) {
                Ok(response) => response,
                Err(_) => continue,
            };
            if response.id == request_id {
                transport::stop_listener();
                return Ok(response);
            }
        }
        if Instant::now() >= deadline {
            transport::stop_listener();
            return Err("timed out waiting for remote rpc response".into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}
