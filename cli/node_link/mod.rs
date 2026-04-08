pub use protocol::*;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use snow::{params::NoiseParams, Builder};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const NODE_LINK_PROTOCOL_VERSION: u8 = 1;

// Pairing bootstrap when peers do not know each other's static keys yet.
pub const NOISE_PAIR_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

// Reconnect path after pairing persists the remote static identity.
pub const NOISE_SESSION_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

pub const NOISE_IMPLEMENTATION: &str = "snow";

// Current runtime uses Tailscale as the network substrate, not raw WireGuard.
pub const TAILSCALE_TRANSPORT_KIND: &str = "tailscale";

// Preferred management path for production is the existing local Tailscale daemon
// via LocalAPI. Embedded libtailscale remains the fallback for future userspace mode.
pub const TAILSCALE_MANAGEMENT_KIND: &str = "daemon-localapi";
pub const TAILSCALE_EMBED_FALLBACK_KIND: &str = "embedded-libtailscale";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeLinkStack {
    pub protocol_version: u8,
    pub transport_kind: String,
    pub tailscale_management_kind: String,
    pub tailscale_embed_fallback_kind: String,
    pub noise_implementation: String,
    pub noise_pair_pattern: String,
    pub noise_session_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLinkIdentityRecord {
    pub version: u8,
    pub key_source: String,
    pub created_at_ms: u64,
    pub public_key: String,
    pub private_key: String,
    pub public_key_id: String,
    pub stack: NodeLinkStack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLinkAdvertisement {
    pub public_key: String,
    pub public_key_id: String,
    pub stack: NodeLinkStack,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteNodeLink {
    pub public_key: String,
    pub public_key_id: String,
    #[serde(default)]
    pub stack: Option<NodeLinkStack>,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairHandshakeProof {
    pub initiator_remote_key_id: String,
    pub responder_remote_key_id: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn stack() -> NodeLinkStack {
    NodeLinkStack {
        protocol_version: NODE_LINK_PROTOCOL_VERSION,
        transport_kind: TAILSCALE_TRANSPORT_KIND.to_string(),
        tailscale_management_kind: TAILSCALE_MANAGEMENT_KIND.to_string(),
        tailscale_embed_fallback_kind: TAILSCALE_EMBED_FALLBACK_KIND.to_string(),
        noise_implementation: NOISE_IMPLEMENTATION.to_string(),
        noise_pair_pattern: NOISE_PAIR_PATTERN.to_string(),
        noise_session_pattern: NOISE_SESSION_PATTERN.to_string(),
    }
}

fn pair_params() -> Result<NoiseParams, Box<dyn std::error::Error>> {
    Ok(NOISE_PAIR_PATTERN.parse()?)
}

#[cfg(test)]
fn session_params() -> Result<NoiseParams, Box<dyn std::error::Error>> {
    Ok(NOISE_SESSION_PATTERN.parse()?)
}

fn identity_path(node: &crate::paths::NodeIdentity) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate::paths::node_dir(&node.label)?.join("node-link.json"))
}

fn encode_key(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
fn decode_key(raw: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(URL_SAFE_NO_PAD.decode(raw.trim())?)
}

fn public_key_id_from_b64(public_key: &str) -> String {
    public_key.chars().take(16).collect()
}

fn write_identity(
    path: &Path,
    identity: &NodeLinkIdentityRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(identity)?;
    fs::write(path, format!("{json}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(path, permissions);
    }
    Ok(())
}

pub fn load_or_create_identity(
    node: &crate::paths::NodeIdentity,
) -> Result<NodeLinkIdentityRecord, Box<dyn std::error::Error>> {
    crate::paths::ensure_node_layout(node)?;
    let path = identity_path(node)?;
    if path.exists() {
        let raw = fs::read_to_string(path)?;
        return Ok(serde_json::from_str(&raw)?);
    }

    let builder = Builder::new(pair_params()?);
    let keypair = builder.generate_keypair()?;
    let public_key = encode_key(&keypair.public);
    let identity = NodeLinkIdentityRecord {
        version: NODE_LINK_PROTOCOL_VERSION,
        key_source: "node-file".to_string(),
        created_at_ms: now_ms(),
        public_key_id: public_key_id_from_b64(&public_key),
        public_key,
        private_key: encode_key(&keypair.private),
        stack: stack(),
    };
    write_identity(&path, &identity)?;
    Ok(identity)
}

pub fn advertise_identity(
    node: &crate::paths::NodeIdentity,
) -> Result<NodeLinkAdvertisement, Box<dyn std::error::Error>> {
    let identity = load_or_create_identity(node)?;
    Ok(NodeLinkAdvertisement {
        public_key: identity.public_key,
        public_key_id: identity.public_key_id,
        stack: identity.stack,
    })
}

pub fn remote_from_advert(advertisement: &NodeLinkAdvertisement) -> RemoteNodeLink {
    RemoteNodeLink {
        public_key: advertisement.public_key.clone(),
        public_key_id: advertisement.public_key_id.clone(),
        stack: Some(advertisement.stack.clone()),
    }
}

pub fn ensure_compatible_stack(
    advertisement: &NodeLinkAdvertisement,
) -> Result<(), Box<dyn std::error::Error>> {
    let local = stack();
    let remote = &advertisement.stack;
    if remote.protocol_version != local.protocol_version {
        return Err(format!(
            "unsupported node-link protocol version: local={} remote={}",
            local.protocol_version, remote.protocol_version
        )
        .into());
    }
    if remote.transport_kind != local.transport_kind {
        return Err(format!(
            "unsupported node-link transport: local={} remote={}",
            local.transport_kind, remote.transport_kind
        )
        .into());
    }
    if remote.noise_implementation != local.noise_implementation
        || remote.noise_pair_pattern != local.noise_pair_pattern
        || remote.noise_session_pattern != local.noise_session_pattern
    {
        return Err("node-link Noise stack is incompatible".into());
    }
    Ok(())
}


mod protocol;
#[cfg(test)]
pub use protocol::*;
