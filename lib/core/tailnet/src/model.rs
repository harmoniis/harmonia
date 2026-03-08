use serde::{Deserialize, Serialize};

/// Tailscale hostname or IP identifying a node on the mesh.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// Capabilities advertised by a mesh node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub frontends: Vec<String>,
    pub tools: Vec<String>,
    pub max_agents: u32,
}

impl NodeCapabilities {
    pub fn to_sexp(&self) -> String {
        let frontends = self
            .frontends
            .iter()
            .map(|f| format!("\"{}\"", f))
            .collect::<Vec<_>>()
            .join(" ");
        let tools = self
            .tools
            .iter()
            .map(|t| format!("\"{}\"", t))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "(capabilities (frontends {}) (tools {}) (max-agents {}))",
            frontends, tools, self.max_agents
        )
    }
}

/// Information about a node on the Tailscale mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub capabilities: NodeCapabilities,
    pub agents: Vec<String>,
    pub last_seen_ms: u64,
}

impl NodeInfo {
    pub fn to_sexp(&self) -> String {
        let agents = self
            .agents
            .iter()
            .map(|a| format!("\"{}\"", a))
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "(node (id \"{}\") {} (agents {}) (last-seen-ms {}))",
            self.id.0,
            self.capabilities.to_sexp(),
            agents,
            self.last_seen_ms
        )
    }
}

/// Type tag for mesh messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeshMessageType {
    Signal,
    Heartbeat,
    Discovery,
    Command,
}

impl MeshMessageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            MeshMessageType::Signal => "signal",
            MeshMessageType::Heartbeat => "heartbeat",
            MeshMessageType::Discovery => "discovery",
            MeshMessageType::Command => "command",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "signal" => Ok(MeshMessageType::Signal),
            "heartbeat" => Ok(MeshMessageType::Heartbeat),
            "discovery" => Ok(MeshMessageType::Discovery),
            "command" => Ok(MeshMessageType::Command),
            other => Err(format!("unknown message type: {}", other)),
        }
    }
}

/// A message sent between nodes on the mesh.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshMessage {
    pub from: String,
    pub to: String,
    pub payload: String,
    pub msg_type: MeshMessageType,
    /// Wave 4.2: Millisecond timestamp for replay protection.
    #[serde(default)]
    pub timestamp_ms: u64,
    /// Wave 4.2: HMAC-SHA256 hex over "from|to|payload|type|timestamp_ms".
    /// Empty string means unsigned (backward compatible).
    #[serde(default)]
    pub hmac: String,
}

impl MeshMessage {
    pub fn to_sexp(&self) -> String {
        format!(
            "(mesh-message (from \"{}\") (to \"{}\") (type {}) (payload \"{}\"))",
            self.from,
            self.to,
            self.msg_type.as_str(),
            self.payload.replace('\\', "\\\\").replace('"', "\\\"")
        )
    }
}
