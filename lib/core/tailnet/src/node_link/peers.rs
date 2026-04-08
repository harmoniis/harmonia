use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct LocalIdentity {
    pub(crate) public_key_id: String,
    pub(crate) private_key: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(crate) struct KnownPeer {
    pub(crate) public_key: Vec<u8>,
    pub(crate) public_key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObservedPeersFile {
    #[serde(default)]
    peers: Vec<ObservedPeer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObservedPeer {
    public_key: String,
    public_key_id: String,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    exact_addrs: Vec<String>,
    #[serde(default)]
    hosts: Vec<String>,
    last_seen_ms: u64,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn data_dir() -> Option<PathBuf> {
    if let Ok(Some(path)) = harmonia_config_store::get_config("tailnet-core", "global", "data-dir")
    {
        if !path.trim().is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".harmoniis").join("harmonia"))
}

fn current_node_label() -> Option<String> {
    if let Ok(Some(label)) = harmonia_config_store::get_config("tailnet-core", "node", "label") {
        if !label.trim().is_empty() {
            return Some(label);
        }
    }
    let path = data_dir()?.join("config").join("node.json");
    let raw = fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    json.get("label")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn current_node_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("nodes").join(current_node_label()?))
}

fn identity_path() -> Option<PathBuf> {
    Some(current_node_dir()?.join("node-link.json"))
}

fn pairings_dir() -> Option<PathBuf> {
    Some(current_node_dir()?.join("pairings"))
}

fn observed_peers_path() -> Option<PathBuf> {
    Some(current_node_dir()?.join("node-link-peers.json"))
}

fn host_from_addr(addr: &str) -> String {
    let trimmed = addr.trim();
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((host, _)) = rest.split_once("]:") {
            return host.to_string();
        }
    }
    if trimmed.matches(':').count() == 1 {
        if let Some((host, _)) = trimmed.rsplit_once(':') {
            return host.to_string();
        }
    }
    trimmed.to_string()
}

pub(crate) fn load_local_identity() -> Option<LocalIdentity> {
    let path = identity_path()?;
    let raw = fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&raw).ok()?;
    let private_key_b64 = json.get("private_key")?.as_str()?.to_string();
    let public_key_id = json.get("public_key_id")?.as_str()?.to_string();
    Some(LocalIdentity {
        public_key_id,
        private_key: URL_SAFE_NO_PAD.decode(private_key_b64).ok()?,
    })
}

fn load_observed_peers() -> Vec<ObservedPeer> {
    let Some(path) = observed_peers_path() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<ObservedPeersFile>(&raw)
        .map(|file| file.peers)
        .unwrap_or_default()
}

fn save_observed_peers(peers: &[ObservedPeer]) -> Result<(), String> {
    let Some(path) = observed_peers_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create peer dir: {}", e))?;
    }
    let file = ObservedPeersFile {
        peers: peers.to_vec(),
    };
    let json =
        serde_json::to_string_pretty(&file).map_err(|e| format!("serialize peers: {}", e))?;
    fs::write(path, format!("{json}\n")).map_err(|e| format!("write peers: {}", e))?;
    Ok(())
}

fn decode_peer_key(raw: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD.decode(raw.trim()).ok()
}

fn load_pairing_peers() -> Vec<(String, Option<String>, KnownPeer)> {
    let Some(dir) = pairings_dir() else {
        return Vec::new();
    };
    let mut peers = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return peers;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let remote_addr = json
            .get("remote_addr")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();
        let remote_label = json
            .get("remote_label")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string());
        let Some(node_link) = json.get("node_link") else {
            continue;
        };
        let Some(public_key_b64) = node_link.get("public_key").and_then(|value| value.as_str())
        else {
            continue;
        };
        let Some(public_key) = decode_peer_key(public_key_b64) else {
            continue;
        };
        let public_key_id = node_link
            .get("public_key_id")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string())
            .unwrap_or_else(|| public_key_b64.chars().take(16).collect());
        peers.push((
            remote_addr,
            remote_label,
            KnownPeer {
                public_key,
                public_key_id,
            },
        ));
    }
    peers
}

pub(crate) fn observed_peer_for_target(target: &str) -> Option<KnownPeer> {
    let host = host_from_addr(target);
    load_observed_peers().into_iter().find_map(|peer| {
        if peer.exact_addrs.iter().any(|value| value == target)
            || peer.hosts.iter().any(|value| value == &host)
        {
            Some(KnownPeer {
                public_key: decode_peer_key(&peer.public_key)?,
                public_key_id: peer.public_key_id,
            })
        } else {
            None
        }
    })
}

pub(crate) fn pairing_peer_for_target(target: &str) -> Option<KnownPeer> {
    let host = host_from_addr(target);
    load_pairing_peers()
        .into_iter()
        .find_map(|(addr, label, peer)| {
            let label_match = label
                .as_deref()
                .map(|value| value == target || value == host)
                .unwrap_or(false);
            if addr == target || host_from_addr(&addr) == host || label_match {
                Some(peer)
            } else {
                None
            }
        })
}

pub(crate) fn known_peer_by_key_id(key_id: &str) -> Option<KnownPeer> {
    if key_id.is_empty() {
        return None;
    }
    for peer in load_observed_peers() {
        if peer.public_key_id == key_id {
            return Some(KnownPeer {
                public_key: decode_peer_key(&peer.public_key)?,
                public_key_id: peer.public_key_id,
            });
        }
    }
    for (_, _, peer) in load_pairing_peers() {
        if peer.public_key_id == key_id {
            return Some(peer);
        }
    }
    None
}

pub(crate) fn remember_peer(
    remote_public_key: &[u8],
    remote_key_id: &str,
    peer_addr: Option<SocketAddr>,
    msg: &crate::model::MeshMessage,
) -> Result<(), String> {
    let mut peers = load_observed_peers();
    let encoded_key = URL_SAFE_NO_PAD.encode(remote_public_key);
    let host = peer_addr.map(|addr| addr.ip().to_string());
    let label = msg
        .origin
        .as_ref()
        .and_then(|origin| origin.node_label.clone())
        .filter(|value| !value.is_empty());
    let mut exact_addrs = Vec::new();
    if !msg.from.is_empty() {
        exact_addrs.push(msg.from.clone());
    }
    if let Some(origin) = &msg.origin {
        if origin.node_id != msg.from && !origin.node_id.is_empty() {
            exact_addrs.push(origin.node_id.clone());
        }
    }

    if let Some(existing) = peers
        .iter_mut()
        .find(|peer| peer.public_key_id == remote_key_id || peer.public_key == encoded_key)
    {
        existing.last_seen_ms = now_ms();
        for addr in exact_addrs {
            if !existing.exact_addrs.contains(&addr) {
                existing.exact_addrs.push(addr);
            }
        }
        if let Some(value) = host {
            if !existing.hosts.contains(&value) {
                existing.hosts.push(value);
            }
        }
        if let Some(value) = label {
            if !existing.labels.contains(&value) {
                existing.labels.push(value);
            }
        }
    } else {
        let mut peer = ObservedPeer {
            public_key: encoded_key,
            public_key_id: remote_key_id.to_string(),
            labels: label.into_iter().collect(),
            exact_addrs,
            hosts: host.into_iter().collect(),
            last_seen_ms: now_ms(),
        };
        peer.exact_addrs.sort();
        peer.exact_addrs.dedup();
        peer.hosts.sort();
        peer.hosts.dedup();
        peer.labels.sort();
        peer.labels.dedup();
        peers.push(peer);
    }

    save_observed_peers(&peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_parser_handles_ipv6() {
        assert_eq!(
            host_from_addr("[fd7a:115c:a1e0::1]:7483"),
            "fd7a:115c:a1e0::1"
        );
        assert_eq!(host_from_addr("100.101.102.103:7483"), "100.101.102.103");
        assert_eq!(host_from_addr("agent-node"), "agent-node");
    }

    #[test]
    fn observed_peer_store_round_trip() {
        let root = std::env::temp_dir().join(format!("harmonia-tailnet-peers-{}", now_ms()));
        fs::create_dir_all(root.join("nodes").join("node-a")).expect("temp node dir");
        std::env::set_var("HARMONIA_DATA_DIR", &root);
        std::env::set_var("HARMONIA_NODE_LABEL", "node-a");

        let peers = vec![ObservedPeer {
            public_key: "abc".to_string(),
            public_key_id: "key-1".to_string(),
            labels: vec!["node-b".to_string()],
            exact_addrs: vec!["node-b:7483".to_string()],
            hosts: vec!["100.64.0.2".to_string()],
            last_seen_ms: now_ms(),
        }];
        save_observed_peers(&peers).expect("save peers");
        let loaded = load_observed_peers();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].public_key_id, "key-1");

        std::env::remove_var("HARMONIA_DATA_DIR");
        std::env::remove_var("HARMONIA_NODE_LABEL");
        let _ = fs::remove_dir_all(root);
    }
}
