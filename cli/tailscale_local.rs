use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tailscale_localapi::types::{PeerStatus, Status};
use tailscale_localapi::LocalApi;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TailnetAdvert {
    pub management_kind: String,
    pub node_id: String,
    pub hostname: String,
    pub dns_name: String,
    pub public_key: String,
    pub tailscale_ips: Vec<String>,
    pub tailnet_name: Option<String>,
    pub backend_state: String,
    pub checked_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TailnetPeer {
    pub management_kind: String,
    pub node_id: String,
    pub hostname: String,
    pub dns_name: String,
    pub public_key: String,
    pub tailscale_ips: Vec<String>,
    pub online: bool,
    pub checked_at_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sanitize_node_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for ch in raw.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            prev_dash = false;
            ch.to_ascii_lowercase()
        } else if matches!(ch, '-' | '_' | '.') {
            prev_dash = false;
            ch
        } else if prev_dash {
            continue;
        } else {
            prev_dash = true;
            '-'
        };
        out.push(mapped);
    }
    let trimmed = out
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.')
        .to_string();
    if trimmed.is_empty() {
        "harmonia-node".to_string()
    } else {
        trimmed
    }
}

fn normalize_dns_name(raw: &str) -> String {
    raw.trim_end_matches('.').to_string()
}

fn dns_host_label(raw: &str) -> String {
    let dns = normalize_dns_name(raw);
    let head = dns.split('.').next().unwrap_or(&dns);
    sanitize_node_label(head)
}

fn host_from_addr(addr: &str) -> String {
    let trimmed = addr.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = rest.split_once("]:") {
            return host.to_string();
        }
    }
    if trimmed.matches(':').count() == 1 {
        return trimmed
            .rsplit_once(':')
            .map(|(host, _)| host.to_string())
            .unwrap_or_else(|| trimmed.to_string());
    }
    trimmed.to_string()
}

fn localapi_runtime() -> Result<tokio::runtime::Runtime, Box<dyn std::error::Error>> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn load_status() -> Result<Option<Status>, Box<dyn std::error::Error>> {
    let socket_path = std::env::var("HARMONIA_TAILSCALE_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/run/tailscale/tailscaled.sock"));
    if !socket_path.exists() {
        return Ok(None);
    }
    let runtime = localapi_runtime()?;
    let status = runtime.block_on(async move {
        let client = LocalApi::new_with_socket_path(socket_path);
        client.status().await
    })?;
    Ok(Some(status))
}

#[cfg(target_os = "macos")]
fn macos_port_and_password() -> Option<(u16, String)> {
    if let (Some(port), Some(password)) = (
        std::env::var("HARMONIA_TAILSCALE_LOCALAPI_PORT")
            .ok()
            .and_then(|raw| raw.parse::<u16>().ok()),
        std::env::var("HARMONIA_TAILSCALE_LOCALAPI_PASSWORD")
            .ok()
            .filter(|raw| !raw.trim().is_empty()),
    ) {
        return Some((port, password));
    }

    let dir = PathBuf::from("/Library/Tailscale");
    let port_path = dir.join("ipnport");
    let port = fs::read_link(port_path)
        .ok()?
        .to_string_lossy()
        .parse()
        .ok()?;
    let password_path = dir.join(format!("sameuserproof-{port}"));
    let password = fs::read_to_string(password_path)
        .ok()?
        .trim_end()
        .to_string();
    if password.is_empty() {
        None
    } else {
        Some((port, password))
    }
}

#[cfg(target_os = "macos")]
fn load_status() -> Result<Option<Status>, Box<dyn std::error::Error>> {
    let Some((port, password)) = macos_port_and_password() else {
        return Ok(None);
    };
    let runtime = localapi_runtime()?;
    let status = runtime.block_on(async move {
        let client = LocalApi::new_with_port_and_password(port, password);
        client.status().await
    })?;
    Ok(Some(status))
}

#[cfg(target_os = "windows")]
fn load_status() -> Result<Option<Status>, Box<dyn std::error::Error>> {
    let Some(port) = std::env::var("HARMONIA_TAILSCALE_LOCALAPI_PORT")
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
    else {
        return Ok(None);
    };
    let Some(password) = std::env::var("HARMONIA_TAILSCALE_LOCALAPI_PASSWORD")
        .ok()
        .filter(|raw| !raw.trim().is_empty())
    else {
        return Ok(None);
    };
    let runtime = localapi_runtime()?;
    let status = runtime.block_on(async move {
        let client = LocalApi::new_with_port_and_password(port, password);
        client.status().await
    })?;
    Ok(Some(status))
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "macos",
    target_os = "windows"
)))]
fn load_status() -> Result<Option<Status>, Box<dyn std::error::Error>> {
    Ok(None)
}

pub fn local_tailnet_advert() -> Result<Option<TailnetAdvert>, Box<dyn std::error::Error>> {
    let Some(status) = load_status()? else {
        return Ok(None);
    };

    Ok(Some(TailnetAdvert {
        management_kind: crate::node_link::TAILSCALE_MANAGEMENT_KIND.to_string(),
        node_id: status.self_status.id,
        hostname: sanitize_node_label(&status.self_status.hostname),
        dns_name: normalize_dns_name(&status.self_status.dnsname),
        public_key: status.self_status.public_key,
        tailscale_ips: status
            .tailscale_ips
            .into_iter()
            .map(|ip| ip.to_string())
            .collect(),
        tailnet_name: status.current_tailnet.map(|tailnet| tailnet.name),
        backend_state: format!("{:?}", status.backend_state),
        checked_at_ms: now_ms(),
    }))
}

fn peer_matches(remote_host: &str, remote_label: &str, peer: &PeerStatus) -> bool {
    let host = sanitize_node_label(remote_host);
    let label = sanitize_node_label(remote_label);
    let peer_host = sanitize_node_label(&peer.hostname);
    let peer_dns = dns_host_label(&peer.dnsname);
    if !host.is_empty() && (peer_host == host || peer_dns == host) {
        return true;
    }
    if !label.is_empty() && (peer_host == label || peer_dns == label) {
        return true;
    }
    peer.tailscale_ips
        .iter()
        .any(|ip| ip.to_string() == remote_host)
}

pub fn find_peer(
    remote_addr: &str,
    remote_label: &str,
) -> Result<Option<TailnetPeer>, Box<dyn std::error::Error>> {
    let Some(status) = load_status()? else {
        return Ok(None);
    };

    let remote_host = host_from_addr(remote_addr);
    for peer in status.peer.into_values() {
        if !peer_matches(&remote_host, remote_label, &peer) {
            continue;
        }
        return Ok(Some(TailnetPeer {
            management_kind: crate::node_link::TAILSCALE_MANAGEMENT_KIND.to_string(),
            node_id: peer.id,
            hostname: sanitize_node_label(&peer.hostname),
            dns_name: normalize_dns_name(&peer.dnsname),
            public_key: peer.public_key,
            tailscale_ips: peer
                .tailscale_ips
                .into_iter()
                .map(|ip| ip.to_string())
                .collect(),
            online: peer.online,
            checked_at_ms: now_ms(),
        }));
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_from_addr_handles_bracketed_ipv6() {
        assert_eq!(
            host_from_addr("[fd7a:115c:a1e0::1]:7483"),
            "fd7a:115c:a1e0::1"
        );
        assert_eq!(host_from_addr("100.101.102.103:7483"), "100.101.102.103");
        assert_eq!(host_from_addr("agent-node"), "agent-node");
    }

    #[test]
    fn normalize_dns_name_drops_trailing_dot() {
        assert_eq!(
            normalize_dns_name("agent-node.tail123.ts.net."),
            "agent-node.tail123.ts.net"
        );
        assert_eq!(dns_host_label("agent-node.tail123.ts.net."), "agent-node");
    }
}
