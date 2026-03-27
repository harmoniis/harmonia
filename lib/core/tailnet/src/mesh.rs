use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{NodeCapabilities, NodeId, NodeInfo};

/// Internal mesh state holding the local node and known peers.
struct MeshState {
    local_node: NodeInfo,
    peers: HashMap<String, NodeInfo>,
    listen_port: u16,
    hostname_prefix: String,
}

/// Deprecated: legacy global singleton. Will be replaced by injected state.
static LEGACY_MESH: OnceLock<RwLock<MeshState>> = OnceLock::new();

fn default_state() -> MeshState {
    let port: u16 = harmonia_config_store::get_own_or("tailnet-core", "port", "7483")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(7483);

    let prefix = harmonia_config_store::get_own_or("tailnet-core", "hostname-prefix", "harmonia-")
        .unwrap_or_else(|_| "harmonia-".to_string());

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| "unknown".to_string());
    let label = harmonia_config_store::get_config("harmonia-cli", "node", "label")
        .ok()
        .flatten()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or_else(|| hostname.clone());
    let advertise_addr =
        harmonia_config_store::get_config("harmonia-cli", "tailnet-core", "advertise-addr")
            .ok()
            .flatten()
            .filter(|raw| !raw.trim().is_empty())
            .unwrap_or_else(|| format!("{}:{}", hostname, port));
    let role = harmonia_config_store::get_config("harmonia-cli", "node", "role")
        .ok()
        .flatten()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or_else(|| "agent".to_string());

    MeshState {
        local_node: NodeInfo {
            id: NodeId(advertise_addr),
            label,
            role,
            capabilities: NodeCapabilities {
                frontends: Vec::new(),
                tools: Vec::new(),
                max_agents: 1,
            },
            agents: Vec::new(),
            last_seen_ms: now_ms(),
        },
        peers: HashMap::new(),
        listen_port: port,
        hostname_prefix: prefix,
    }
}

fn mesh() -> &'static RwLock<MeshState> {
    LEGACY_MESH.get_or_init(|| RwLock::new(default_state()))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Parse a minimal s-expression config and initialise the local node.
///
/// Expected format (all fields optional):
/// ```text
/// (tailnet-config
///   (id "my-hostname")
///   (port 7483)
///   (prefix "harmonia-")
///   (max-agents 4)
///   (frontends "tui" "mqtt")
///   (tools "browser" "search"))
/// ```
pub fn init(config_sexp: &str) -> Result<(), String> {
    let mut state = mesh().write().map_err(|e| format!("lock: {}", e))?;

    // Simple s-expression key extraction helpers.
    if let Some(id) = extract_sexp_string(config_sexp, "id") {
        state.local_node.id = NodeId(id);
    }
    if let Some(label) = extract_sexp_string(config_sexp, "label") {
        state.local_node.label = label;
    }
    if let Some(role) = extract_sexp_string(config_sexp, "role") {
        state.local_node.role = role;
    }
    if let Some(port) = extract_sexp_u16(config_sexp, "port") {
        state.listen_port = port;
    }
    if let Some(prefix) = extract_sexp_string(config_sexp, "prefix") {
        state.hostname_prefix = prefix;
    }
    if let Some(max) = extract_sexp_u32(config_sexp, "max-agents") {
        state.local_node.capabilities.max_agents = max;
    }
    if let Some(list) = extract_sexp_string_list(config_sexp, "frontends") {
        state.local_node.capabilities.frontends = list;
    }
    if let Some(list) = extract_sexp_string_list(config_sexp, "tools") {
        state.local_node.capabilities.tools = list;
    }

    state.local_node.last_seen_ms = now_ms();

    log::info!(
        "tailnet mesh initialised: id={} port={}",
        state.local_node.id.0,
        state.listen_port
    );

    Ok(())
}

/// Return all known peers as a list.
pub fn discover_peers() -> Result<Vec<NodeInfo>, String> {
    let state = mesh().read().map_err(|e| format!("lock: {}", e))?;
    Ok(state.peers.values().cloned().collect())
}

/// Register or update a peer node.
pub fn register_node(node_info_sexp: &str) -> Result<(), String> {
    // Parse minimal fields from sexp.
    let id = extract_sexp_string(node_info_sexp, "id")
        .ok_or_else(|| "missing (id ...) in node info".to_string())?;
    let label = extract_sexp_string(node_info_sexp, "label").unwrap_or_else(|| id.clone());
    let role = extract_sexp_string(node_info_sexp, "role").unwrap_or_else(|| "agent".to_string());

    let frontends = extract_sexp_string_list(node_info_sexp, "frontends").unwrap_or_default();
    let tools = extract_sexp_string_list(node_info_sexp, "tools").unwrap_or_default();
    let max_agents = extract_sexp_u32(node_info_sexp, "max-agents").unwrap_or(1);
    let agents = extract_sexp_string_list(node_info_sexp, "agents").unwrap_or_default();

    let info = NodeInfo {
        id: NodeId(id.clone()),
        label,
        role,
        capabilities: NodeCapabilities {
            frontends,
            tools,
            max_agents,
        },
        agents,
        last_seen_ms: now_ms(),
    };

    let mut state = mesh().write().map_err(|e| format!("lock: {}", e))?;
    state.peers.insert(id, info);
    Ok(())
}

/// Return the local node info as an s-expression string.
pub fn local_node_info() -> Result<String, String> {
    let state = mesh().read().map_err(|e| format!("lock: {}", e))?;
    Ok(state.local_node.to_sexp())
}

pub fn local_node() -> Result<NodeInfo, String> {
    let state = mesh().read().map_err(|e| format!("lock: {}", e))?;
    Ok(state.local_node.clone())
}

/// Return the configured listen port.
pub fn listen_port() -> u16 {
    mesh().read().map(|s| s.listen_port).unwrap_or(7483)
}

// ---------------------------------------------------------------------------
// Minimal s-expression extractors (no full parser needed)
// ---------------------------------------------------------------------------

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pattern = format!("({} \"", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_sexp_u16(sexp: &str, key: &str) -> Option<u16> {
    let pattern = format!("({} ", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find(')')?.min(rest.find(' ').unwrap_or(rest.len()));
    rest[..end].trim().parse().ok()
}

fn extract_sexp_u32(sexp: &str, key: &str) -> Option<u32> {
    let pattern = format!("({} ", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find(')')?.min(rest.find(' ').unwrap_or(rest.len()));
    rest[..end].trim().parse().ok()
}

fn extract_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("({} ", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find(')')?;
    let segment = &rest[..end];
    let items: Vec<String> = segment
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}
