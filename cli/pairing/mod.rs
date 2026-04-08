use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingInvite {
    pub version: u8,
    pub kind: String,
    pub invite_id: String,
    pub node_label: String,
    pub node_role: String,
    pub connect_addr: String,
    pub issued_at_ms: u64,
    #[serde(default)]
    pub node_link: Option<crate::node_link::NodeLinkAdvertisement>,
    #[serde(default)]
    pub tailnet: Option<crate::tailscale_local::TailnetAdvert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRecord {
    pub version: u8,
    pub remote_label: String,
    pub remote_role: String,
    pub remote_addr: String,
    pub invite_id: String,
    pub paired_at_ms: u64,
    #[serde(default)]
    pub grants: Vec<String>,
    #[serde(default)]
    pub node_link: Option<crate::node_link::RemoteNodeLink>,
    #[serde(default)]
    pub tailnet: Option<crate::tailscale_local::TailnetPeer>,
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn tailnet_port() -> u16 {
    crate::paths::config_value("tailnet-core", "port")
        .ok()
        .flatten()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(7483)
}

pub(super) fn default_grants_for_roles(local_role: crate::paths::NodeRole, remote_role: &str) -> Vec<String> {
    let mut grants = vec![
        harmonia_node_rpc::capability::PING.to_string(),
        harmonia_node_rpc::capability::CAPABILITIES.to_string(),
        harmonia_node_rpc::capability::FS_LIST.to_string(),
        harmonia_node_rpc::capability::FS_READ_TEXT.to_string(),
        harmonia_node_rpc::capability::WALLET_STATUS.to_string(),
        harmonia_node_rpc::capability::WALLET_LIST_SYMBOLS.to_string(),
        harmonia_node_rpc::capability::WALLET_HAS_SYMBOL.to_string(),
    ];

    let remote_role = remote_role.trim().to_ascii_lowercase();
    if matches!(
        local_role,
        crate::paths::NodeRole::Agent | crate::paths::NodeRole::TuiClient
    ) || remote_role == "agent"
    {
        grants.extend([
            harmonia_node_rpc::capability::SHELL_EXEC.to_string(),
            harmonia_node_rpc::capability::TMUX_LIST.to_string(),
            harmonia_node_rpc::capability::TMUX_SPAWN.to_string(),
            harmonia_node_rpc::capability::TMUX_CAPTURE.to_string(),
            harmonia_node_rpc::capability::TMUX_SEND_LINE.to_string(),
            harmonia_node_rpc::capability::TMUX_SEND_KEY.to_string(),
            harmonia_node_rpc::capability::WALLET_SET_SECRET.to_string(),
            harmonia_node_rpc::capability::FRONTEND_PAIR_LIST.to_string(),
            harmonia_node_rpc::capability::FRONTEND_CONFIGURE.to_string(),
            harmonia_node_rpc::capability::FRONTEND_PAIR_INIT.to_string(),
            harmonia_node_rpc::capability::FRONTEND_PAIR_STATUS.to_string(),
        ]);
    }

    grants.sort();
    grants.dedup();
    grants
}

pub fn pairing_from_mesh_message(
    node: &crate::paths::NodeIdentity,
    msg: &harmonia_tailnet::model::MeshMessage,
) -> PairingRecord {
    let origin = msg.origin.as_ref();
    let remote_addr = origin
        .map(|origin| origin.node_id.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| msg.from.clone());
    let remote_label = origin
        .and_then(|origin| origin.node_label.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| remote_addr.clone());
    let remote_role = origin
        .and_then(|origin| origin.node_role.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    PairingRecord {
        version: 1,
        remote_label,
        remote_role: remote_role.clone(),
        remote_addr,
        invite_id: format!("mesh-{}", now_ms()),
        paired_at_ms: now_ms(),
        grants: default_grants_for_roles(node.role, &remote_role),
        node_link: None,
        tailnet: None,
    }
}

fn resolve_pairing_grants(
    node: &crate::paths::NodeIdentity,
    remote_role: &str,
    current: Vec<String>,
) -> Vec<String> {
    let mut grants = if current.is_empty() {
        default_grants_for_roles(node.role, remote_role)
    } else {
        current
    };
    grants.sort();
    grants.dedup();
    grants
}

fn tailnet_advertise_host(node: &crate::paths::NodeIdentity) -> String {
    crate::paths::config_value("tailnet-core", "advertise-host")
        .ok()
        .flatten()
        .filter(|raw| !raw.trim().is_empty())
        .or_else(|| {
            crate::paths::config_value("tailnet-core", "advertise-addr")
                .ok()
                .flatten()
        })
        .map(|raw| {
            if raw.contains(':') {
                raw.split(':').next().unwrap_or(&raw).to_string()
            } else {
                raw
            }
        })
        .unwrap_or_else(|| node.hostname.clone())
}

pub fn advertised_addr(node: &crate::paths::NodeIdentity) -> String {
    let addr = crate::paths::config_value("tailnet-core", "advertise-addr")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{}:{}", tailnet_advertise_host(node), tailnet_port()));
    sync_tailnet_runtime(node, &addr);
    addr
}

pub(super) fn sync_tailnet_runtime(node: &crate::paths::NodeIdentity, advertise_addr: &str) {
    let _ = crate::paths::set_config_value("tailnet-core", "port", &tailnet_port().to_string());
    let _ = crate::paths::set_config_value("tailnet-core", "advertise-addr", advertise_addr);
    let _ = crate::paths::set_config_value(
        "tailnet-core",
        "advertise-host",
        &tailnet_advertise_host(node),
    );
}

fn default_pairing_path(
    node: &crate::paths::NodeIdentity,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate::paths::node_pairings_dir(&node.label)?.join("default.json"))
}

pub fn load_default_pairing(
    node: &crate::paths::NodeIdentity,
) -> Result<Option<PairingRecord>, Box<dyn std::error::Error>> {
    let path = default_pairing_path(node)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let mut pairing: PairingRecord = serde_json::from_str(&raw)?;
    pairing.grants = resolve_pairing_grants(node, &pairing.remote_role, pairing.grants);
    Ok(Some(pairing))
}

pub fn save_default_pairing(
    node: &crate::paths::NodeIdentity,
    pairing: &PairingRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = default_pairing_path(node)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(pairing)?),
    )?;
    Ok(())
}


mod encoding;
pub use encoding::*;
