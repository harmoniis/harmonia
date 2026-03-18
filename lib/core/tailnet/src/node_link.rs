use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use snow::{params::NoiseParams, Builder};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const NODE_LINK_MAGIC: &[u8; 4] = b"HNL1";
const MODE_XX: u8 = 1;
const MODE_IK: u8 = 2;
const NOISE_XX_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
const NOISE_IK_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub transport_security: &'static str,
    pub remote_key_id: String,
}

#[derive(Debug, Clone)]
struct LocalIdentity {
    public_key_id: String,
    private_key: Vec<u8>,
}

#[derive(Debug, Clone)]
struct KnownPeer {
    public_key: Vec<u8>,
    public_key_id: String,
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

fn now_ms() -> u64 {
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

fn parse_noise_params(pattern: &str) -> Result<NoiseParams, String> {
    pattern.parse().map_err(|e| format!("noise params: {}", e))
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

fn load_local_identity() -> Option<LocalIdentity> {
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

fn observed_peer_for_target(target: &str) -> Option<KnownPeer> {
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

fn pairing_peer_for_target(target: &str) -> Option<KnownPeer> {
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

fn known_peer_by_key_id(key_id: &str) -> Option<KnownPeer> {
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

fn remember_peer(
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

fn write_u16(stream: &mut TcpStream, value: u16) -> Result<(), String> {
    stream
        .write_all(&value.to_be_bytes())
        .map_err(|e| format!("write u16: {}", e))
}

fn read_u16(stream: &mut TcpStream) -> Result<u16, String> {
    let mut buf = [0u8; 2];
    stream
        .read_exact(&mut buf)
        .map_err(|e| format!("read u16: {}", e))?;
    Ok(u16::from_be_bytes(buf))
}

fn write_blob(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), String> {
    let len = u32::try_from(bytes.len()).map_err(|_| "blob too large".to_string())?;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("write blob len: {}", e))?;
    stream
        .write_all(bytes)
        .map_err(|e| format!("write blob body: {}", e))
}

fn read_blob(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read blob len: {}", e))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 16 * 1024 * 1024 {
        return Err(format!("node-link blob too large: {}", len));
    }
    let mut body = vec![0u8; len];
    stream
        .read_exact(&mut body)
        .map_err(|e| format!("read blob body: {}", e))?;
    Ok(body)
}

fn write_secure_header(
    stream: &mut TcpStream,
    mode: u8,
    sender_key_id: &str,
) -> Result<(), String> {
    stream
        .write_all(NODE_LINK_MAGIC)
        .map_err(|e| format!("write node-link magic: {}", e))?;
    stream
        .write_all(&[mode])
        .map_err(|e| format!("write node-link mode: {}", e))?;
    let sender = sender_key_id.as_bytes();
    let len = u16::try_from(sender.len()).map_err(|_| "sender key id too large".to_string())?;
    write_u16(stream, len)?;
    stream
        .write_all(sender)
        .map_err(|e| format!("write sender key id: {}", e))
}

fn read_secure_header(stream: &mut TcpStream) -> Result<(u8, String), String> {
    let mut mode = [0u8; 1];
    stream
        .read_exact(&mut mode)
        .map_err(|e| format!("read node-link mode: {}", e))?;
    let sender_len = read_u16(stream)? as usize;
    let mut sender_buf = vec![0u8; sender_len];
    if sender_len > 0 {
        stream
            .read_exact(&mut sender_buf)
            .map_err(|e| format!("read sender key id: {}", e))?;
    }
    let sender_key_id =
        String::from_utf8(sender_buf).map_err(|e| format!("sender key id utf8: {}", e))?;
    Ok((mode[0], sender_key_id))
}

fn secure_send_xx(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    expected_remote: Option<&KnownPeer>,
    payload: &[u8],
) -> Result<(), String> {
    write_secure_header(stream, MODE_XX, &local.public_key_id)?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_XX_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .build_initiator()
        .map_err(|e| format!("noise init xx: {}", e))?;
    let mut buffer = vec![0u8; payload.len() + 1024];
    let len1 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write1: {}", e))?;
    write_blob(stream, &buffer[..len1])?;

    let frame2 = read_blob(stream)?;
    handshake
        .read_message(&frame2, &mut buffer)
        .map_err(|e| format!("noise xx read2: {}", e))?;
    if let Some(expected) = expected_remote {
        let observed = handshake
            .get_remote_static()
            .ok_or_else(|| "noise xx missing responder static".to_string())?;
        if observed != expected.public_key.as_slice() {
            return Err("noise xx responder key mismatch".to_string());
        }
    }

    let len3 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write3: {}", e))?;
    write_blob(stream, &buffer[..len3])?;

    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise xx transport: {}", e))?;
    let cipher_len = transport
        .write_message(payload, &mut buffer)
        .map_err(|e| format!("noise xx encrypt: {}", e))?;
    write_blob(stream, &buffer[..cipher_len])
}

fn secure_send_ik(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    remote: &KnownPeer,
    payload: &[u8],
) -> Result<(), String> {
    write_secure_header(stream, MODE_IK, &local.public_key_id)?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_IK_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .remote_public_key(&remote.public_key)
        .map_err(|e| format!("noise remote key: {}", e))?
        .build_initiator()
        .map_err(|e| format!("noise init ik: {}", e))?;
    let mut buffer = vec![0u8; payload.len() + 1024];
    let len1 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise ik write1: {}", e))?;
    write_blob(stream, &buffer[..len1])?;

    let frame2 = read_blob(stream)?;
    handshake
        .read_message(&frame2, &mut buffer)
        .map_err(|e| format!("noise ik read2: {}", e))?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise ik transport: {}", e))?;
    let cipher_len = transport
        .write_message(payload, &mut buffer)
        .map_err(|e| format!("noise ik encrypt: {}", e))?;
    write_blob(stream, &buffer[..cipher_len])
}

fn recv_secure_xx(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    sender_key_id: &str,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let mut handshake = Builder::new(parse_noise_params(NOISE_XX_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .build_responder()
        .map_err(|e| format!("noise resp xx: {}", e))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let frame1 = read_blob(stream)?;
    handshake
        .read_message(&frame1, &mut buffer)
        .map_err(|e| format!("noise xx read1: {}", e))?;
    let len2 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise xx write2: {}", e))?;
    write_blob(stream, &buffer[..len2])?;
    let frame3 = read_blob(stream)?;
    handshake
        .read_message(&frame3, &mut buffer)
        .map_err(|e| format!("noise xx read3: {}", e))?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise xx transport: {}", e))?;
    let remote_static = transport
        .get_remote_static()
        .ok_or_else(|| "noise xx missing initiator static".to_string())?
        .to_vec();
    let remote_key_id: String = URL_SAFE_NO_PAD
        .encode(&remote_static)
        .chars()
        .take(16)
        .collect();
    if !sender_key_id.is_empty() && sender_key_id != remote_key_id {
        return Err("noise xx sender key id mismatch".to_string());
    }
    let ciphertext = read_blob(stream)?;
    let plain_len = transport
        .read_message(&ciphertext, &mut buffer)
        .map_err(|e| format!("noise xx decrypt: {}", e))?;
    Ok((
        buffer[..plain_len].to_vec(),
        SecurityContext {
            transport_security: "noise-xx",
            remote_key_id,
        },
        remote_static,
    ))
}

fn recv_secure_ik(
    stream: &mut TcpStream,
    local: &LocalIdentity,
    sender_key_id: &str,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let sender = known_peer_by_key_id(sender_key_id)
        .ok_or_else(|| format!("noise ik unknown sender key id: {}", sender_key_id))?;
    let mut handshake = Builder::new(parse_noise_params(NOISE_IK_PATTERN)?)
        .local_private_key(&local.private_key)
        .map_err(|e| format!("noise local key: {}", e))?
        .remote_public_key(&sender.public_key)
        .map_err(|e| format!("noise remote key: {}", e))?
        .build_responder()
        .map_err(|e| format!("noise resp ik: {}", e))?;
    let mut buffer = vec![0u8; 64 * 1024];
    let frame1 = read_blob(stream)?;
    handshake
        .read_message(&frame1, &mut buffer)
        .map_err(|e| format!("noise ik read1: {}", e))?;
    let len2 = handshake
        .write_message(&[], &mut buffer)
        .map_err(|e| format!("noise ik write2: {}", e))?;
    write_blob(stream, &buffer[..len2])?;
    let mut transport = handshake
        .into_transport_mode()
        .map_err(|e| format!("noise ik transport: {}", e))?;
    let remote_static = transport
        .get_remote_static()
        .ok_or_else(|| "noise ik missing initiator static".to_string())?
        .to_vec();
    let ciphertext = read_blob(stream)?;
    let plain_len = transport
        .read_message(&ciphertext, &mut buffer)
        .map_err(|e| format!("noise ik decrypt: {}", e))?;
    Ok((
        buffer[..plain_len].to_vec(),
        SecurityContext {
            transport_security: "noise-ik",
            remote_key_id: sender.public_key_id.clone(),
        },
        remote_static,
    ))
}

pub fn write_secure_message(
    stream: &mut TcpStream,
    to_addr: &str,
    message_bytes: &[u8],
) -> Result<bool, String> {
    let Some(local) = load_local_identity() else {
        return Ok(false);
    };

    if let Some(peer) = observed_peer_for_target(to_addr) {
        secure_send_ik(stream, &local, &peer, message_bytes)?;
        return Ok(true);
    }

    if let Some(peer) = pairing_peer_for_target(to_addr) {
        secure_send_xx(stream, &local, Some(&peer), message_bytes)?;
        return Ok(true);
    }

    Ok(false)
}

fn read_secure_message(
    stream: &mut TcpStream,
    local: &LocalIdentity,
) -> Result<(Vec<u8>, SecurityContext, Vec<u8>), String> {
    let (mode, sender_key_id) = read_secure_header(stream)?;
    match mode {
        MODE_XX => recv_secure_xx(stream, local, &sender_key_id),
        MODE_IK => recv_secure_ik(stream, local, &sender_key_id),
        other => Err(format!("unknown node-link mode: {}", other)),
    }
}

pub fn secure_or_plain_body(
    stream: &mut TcpStream,
    first4: [u8; 4],
) -> Result<(Vec<u8>, Option<SecurityContext>), String> {
    if first4 == *NODE_LINK_MAGIC {
        let local = load_local_identity().ok_or_else(|| {
            "secure node-link message received but local node-link identity is missing".to_string()
        })?;
        let peer_addr = stream.peer_addr().ok();
        let (body, security, remote_static) = read_secure_message(stream, &local)?;
        let msg: crate::model::MeshMessage =
            serde_json::from_slice(&body).map_err(|e| format!("deserialize secure mesh: {}", e))?;
        remember_peer(&remote_static, &security.remote_key_id, peer_addr, &msg)?;
        Ok((body, Some(security)))
    } else {
        let len = u32::from_be_bytes(first4) as usize;
        if len > 16 * 1024 * 1024 {
            return Err(format!("message too large: {} bytes", len));
        }
        let mut body = vec![0u8; len];
        stream
            .read_exact(&mut body)
            .map_err(|e| format!("read body: {}", e))?;
        Ok((body, None))
    }
}

pub fn apply_security_context(msg: &mut crate::model::MeshMessage, security: &SecurityContext) {
    let origin = msg.origin.get_or_insert_with(|| crate::model::MeshOrigin {
        node_id: msg.from.clone(),
        node_label: None,
        node_role: None,
        channel_class: None,
        node_key_id: None,
        transport_security: None,
    });
    origin.node_key_id = Some(security.remote_key_id.clone());
    origin.transport_security = Some(security.transport_security.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use snow::Builder as SnowBuilder;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

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

    fn test_identity() -> (LocalIdentity, Vec<u8>) {
        let builder = SnowBuilder::new(parse_noise_params(NOISE_XX_PATTERN).expect("noise params"));
        let keypair = builder.generate_keypair().expect("keypair");
        let encoded = URL_SAFE_NO_PAD.encode(&keypair.public);
        (
            LocalIdentity {
                public_key_id: encoded.chars().take(16).collect(),
                private_key: keypair.private,
            },
            keypair.public,
        )
    }

    #[test]
    fn secure_xx_wire_round_trip() {
        let (server_identity, server_public) = test_identity();
        let (client_identity, _) = test_identity();

        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");

        let server_thread = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut magic = [0u8; 4];
            stream.read_exact(&mut magic).expect("read magic");
            assert_eq!(&magic, NODE_LINK_MAGIC);
            let (mode, sender_key_id) = read_secure_header(&mut stream).expect("header");
            assert_eq!(mode, MODE_XX);
            let (body, security, _) =
                recv_secure_xx(&mut stream, &server_identity, &sender_key_id).expect("recv xx");
            assert_eq!(security.transport_security, "noise-xx");
            assert!(!security.remote_key_id.is_empty());
            body
        });

        let mut client_stream = TcpStream::connect(addr).expect("connect");
        let known_server = KnownPeer {
            public_key: server_public,
            public_key_id: String::new(),
        };
        secure_send_xx(
            &mut client_stream,
            &client_identity,
            Some(&known_server),
            b"hello secure tailnet",
        )
        .expect("send xx");

        let body = server_thread.join().expect("join server");
        assert_eq!(body, b"hello secure tailnet");
    }
}
