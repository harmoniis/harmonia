use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use dialoguer::Input;

use super::{
    advertised_addr, default_grants_for_roles, load_default_pairing, now_ms,
    save_default_pairing, sync_tailnet_runtime, PairingInvite, PairingRecord,
};

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
