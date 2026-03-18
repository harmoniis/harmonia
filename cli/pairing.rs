use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use dialoguer::Input;
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

fn now_ms() -> u64 {
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

fn default_grants_for_roles(local_role: crate::paths::NodeRole, remote_role: &str) -> Vec<String> {
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
        .or_else(|| crate::paths::config_value("tailnet-core", "advertise-addr").ok().flatten())
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

fn sync_tailnet_runtime(node: &crate::paths::NodeIdentity, advertise_addr: &str) {
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

pub fn encode_invite(invite: &PairingInvite) -> Result<String, Box<dyn std::error::Error>> {
    let json = serde_json::to_vec(invite)?;
    Ok(URL_SAFE_NO_PAD.encode(json))
}

pub fn decode_invite(code: &str) -> Result<PairingInvite, Box<dyn std::error::Error>> {
    let bytes = URL_SAFE_NO_PAD.decode(code.trim())?;
    let invite: PairingInvite = serde_json::from_slice(&bytes)?;
    if invite.kind != "harmonia-pairing" {
        return Err(format!("unsupported pairing code kind: {}", invite.kind).into());
    }
    if let Some(node_link) = &invite.node_link {
        crate::node_link::ensure_compatible_stack(node_link)?;
    }
    Ok(invite)
}

pub fn generate_invite(
    node: &crate::paths::NodeIdentity,
) -> Result<PairingInvite, Box<dyn std::error::Error>> {
    sync_tailnet_runtime(node, &advertised_addr(node));
    Ok(PairingInvite {
        version: 1,
        kind: "harmonia-pairing".to_string(),
        invite_id: format!("invite-{}", now_ms()),
        node_label: node.label.clone(),
        node_role: node.role.as_str().to_string(),
        connect_addr: advertised_addr(node),
        issued_at_ms: now_ms(),
        node_link: Some(crate::node_link::advertise_identity(node)?),
        tailnet: crate::tailscale_local::local_tailnet_advert()?,
    })
}

pub fn pairing_from_invite(
    local_node: &crate::paths::NodeIdentity,
    invite: PairingInvite,
) -> Result<PairingRecord, Box<dyn std::error::Error>> {
    let remote_role = invite.node_role.clone();
    let tailnet = crate::tailscale_local::find_peer(&invite.connect_addr, &invite.node_label)?
        .or_else(|| {
            invite
                .tailnet
                .as_ref()
                .map(|tailnet| crate::tailscale_local::TailnetPeer {
                    management_kind: tailnet.management_kind.clone(),
                    node_id: tailnet.node_id.clone(),
                    hostname: tailnet.hostname.clone(),
                    dns_name: tailnet.dns_name.clone(),
                    public_key: tailnet.public_key.clone(),
                    tailscale_ips: tailnet.tailscale_ips.clone(),
                    online: false,
                    checked_at_ms: tailnet.checked_at_ms,
                })
        });

    Ok(PairingRecord {
        version: invite.version,
        remote_label: invite.node_label,
        remote_role: remote_role.clone(),
        remote_addr: invite.connect_addr,
        invite_id: invite.invite_id,
        paired_at_ms: now_ms(),
        grants: default_grants_for_roles(local_node.role, &remote_role),
        node_link: invite
            .node_link
            .as_ref()
            .map(crate::node_link::remote_from_advert),
        tailnet,
    })
}

pub fn ensure_pairing(
    node: &crate::paths::NodeIdentity,
) -> Result<PairingRecord, Box<dyn std::error::Error>> {
    let _ = crate::node_link::load_or_create_identity(node)?;
    if let Some(existing) = load_default_pairing(node)? {
        return Ok(existing);
    }

    let code = if let Some(code) = crate::paths::config_value("node", "pair-code")
        .ok()
        .flatten()
        .filter(|raw| !raw.trim().is_empty())
    {
        code
    } else {
        Input::<String>::new()
            .with_prompt("Paste pairing code from the remote agent (`harmonia pairing invite`)")
            .interact_text()?
    };
    let invite = decode_invite(&code)?;
    let pairing = pairing_from_invite(node, invite)?;
    save_default_pairing(node, &pairing)?;
    let _ = harmonia_config_store::delete_config("harmonia-cli", "node", "pair-code");
    Ok(pairing)
}

pub fn print_invite(node: &crate::paths::NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    let invite = generate_invite(node)?;
    let code = encode_invite(&invite)?;
    println!("Pairing invite");
    println!("  node:    {}", invite.node_label);
    println!("  role:    {}", invite.node_role);
    println!("  address: {}", invite.connect_addr);
    if let Some(node_link) = &invite.node_link {
        println!("  key id:  {}", node_link.public_key_id);
        println!("  noise:   {}", node_link.stack.noise_pair_pattern);
    }
    if let Some(tailnet) = &invite.tailnet {
        println!(
            "  tailnet: {}",
            tailnet.tailnet_name.as_deref().unwrap_or("connected")
        );
        println!("  dns:     {}", tailnet.dns_name);
    } else {
        println!("  tailnet: local tailscaled status unavailable");
    }
    println!();
    println!("Make sure the remote agent is running with the tailscale frontend enabled.");
    println!();
    println!("{}", code);
    if let Ok(qr) = harmonia_qr_terminal::render_qr_to_string(&code) {
        println!("\n{}", qr);
    }
    Ok(())
}

pub fn print_pairing(node: &crate::paths::NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    match load_default_pairing(node)? {
        Some(pairing) => {
            println!("Current pairing");
            println!("  remote:  {}", pairing.remote_label);
            println!("  role:    {}", pairing.remote_role);
            println!("  address: {}", pairing.remote_addr);
            println!("  invite:  {}", pairing.invite_id);
            if !pairing.grants.is_empty() {
                println!("  grants:  {}", pairing.grants.join(", "));
            }
            if let Some(node_link) = &pairing.node_link {
                println!("  key id:  {}", node_link.public_key_id);
                if let Some(stack) = &node_link.stack {
                    println!("  noise:   {}", stack.noise_session_pattern);
                }
            }
            if let Some(tailnet) = &pairing.tailnet {
                println!("  dns:     {}", tailnet.dns_name);
                println!("  online:  {}", if tailnet.online { "yes" } else { "no" });
            }
        }
        None => {
            println!("No pairing saved for node {}", node.label);
        }
    }
    Ok(())
}
