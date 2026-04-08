use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProfile {
    FullAgent,
    TuiClient,
}

impl InstallProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FullAgent => "full-agent",
            Self::TuiClient => "tui-client",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tui-client" | "client" | "tui" => Self::TuiClient,
            _ => Self::FullAgent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeRole {
    Agent,
    TuiClient,
    MqttClient,
}

impl NodeRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::TuiClient => "tui-client",
            Self::MqttClient => "mqtt-client",
        }
    }

    pub fn default_for_profile(profile: InstallProfile) -> Self {
        match profile {
            InstallProfile::FullAgent => Self::Agent,
            InstallProfile::TuiClient => Self::TuiClient,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub label: String,
    pub hostname: String,
    pub role: NodeRole,
    pub install_profile: InstallProfile,
}

fn detect_hostname() -> String {
    std::env::var("HARMONIA_NODE_LABEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("HOST").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "harmonia-node".to_string())
}

pub(crate) fn sanitize_node_label(raw: &str) -> String {
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

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

pub fn node_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(super::data_dir()?.join("config").join("node.json"))
}

pub fn nodes_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = super::data_dir()?.join("nodes");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(nodes_root()?.join(sanitize_node_label(label)))
}

pub fn node_sessions_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_pairings_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("pairings");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_memory_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("memory");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn node_manifest_path(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(node_dir(label)?.join("node.json"))
}

pub fn detect_install_profile() -> InstallProfile {
    std::env::var("HARMONIA_INSTALL_PROFILE")
        .map(|raw| InstallProfile::parse(&raw))
        .or_else(|_| {
            super::config_value("node", "install-profile")
                .ok()
                .flatten()
                .map(|raw| InstallProfile::parse(&raw))
                .ok_or(std::env::VarError::NotPresent)
        })
        .unwrap_or(InstallProfile::FullAgent)
}

pub fn ensure_node_layout(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(super::data_dir()?)?;
    fs::create_dir_all(super::data_dir()?.join("config"))?;
    fs::create_dir_all(node_dir(&identity.label)?)?;
    fs::create_dir_all(node_sessions_dir(&identity.label)?)?;
    fs::create_dir_all(node_pairings_dir(&identity.label)?)?;
    fs::create_dir_all(node_memory_dir(&identity.label)?)?;
    Ok(())
}

pub fn persist_node_identity(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    ensure_node_layout(identity)?;
    write_json(&node_config_path()?, identity)?;
    write_json(&node_manifest_path(&identity.label)?, identity)?;
    Ok(())
}

pub fn sync_runtime_config(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    let state_root = super::config::ensure_state_root_env()?;
    let lib_dir = super::lib_dir()?;
    let share_dir = super::share_dir()?;
    let run_dir = super::run_dir()?;
    let log_dir = super::log_dir()?;
    let wallet_root = super::wallet_root_path()?;
    let wallet_db = super::wallet_db_path()?;
    let vault_db = super::vault_db_path()?;

    let entries = [
        ("global", "state-root", state_root.to_string_lossy().to_string()),
        ("global", "system-dir", state_root.to_string_lossy().to_string()),
        ("global", "data-dir", state_root.to_string_lossy().to_string()),
        ("global", "lib-dir", lib_dir.to_string_lossy().to_string()),
        ("global", "share-dir", share_dir.to_string_lossy().to_string()),
        ("global", "run-dir", run_dir.to_string_lossy().to_string()),
        ("global", "log-dir", log_dir.to_string_lossy().to_string()),
        ("global", "wallet-root", wallet_root.to_string_lossy().to_string()),
        ("global", "wallet-db", wallet_db.to_string_lossy().to_string()),
        ("global", "vault-db", vault_db.to_string_lossy().to_string()),
        ("node", "label", identity.label.clone()),
        ("node", "hostname", identity.hostname.clone()),
        ("node", "role", identity.role.as_str().to_string()),
        ("node", "install-profile", identity.install_profile.as_str().to_string()),
        ("node", "sessions-root", node_sessions_dir(&identity.label)?.to_string_lossy().to_string()),
        ("node", "pairings-root", node_pairings_dir(&identity.label)?.to_string_lossy().to_string()),
        ("node", "memory-root", node_memory_dir(&identity.label)?.to_string_lossy().to_string()),
    ];
    for (scope, key, value) in entries {
        super::set_config_value(scope, key, &value)?;
    }
    std::env::set_var(
        "HARMONIA_WALLET_ROOT",
        wallet_root.to_string_lossy().as_ref(),
    );
    std::env::set_var(
        "HARMONIA_VAULT_WALLET_DB",
        wallet_db.to_string_lossy().as_ref(),
    );
    Ok(())
}

pub fn load_node_identity() -> Result<Option<NodeIdentity>, Box<dyn std::error::Error>> {
    let path = node_config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn current_node_identity() -> Result<NodeIdentity, Box<dyn std::error::Error>> {
    if let Some(identity) = load_node_identity()? {
        ensure_node_layout(&identity)?;
        sync_runtime_config(&identity)?;
        return Ok(identity);
    }

    let install_profile = detect_install_profile();
    let hostname = detect_hostname();
    let identity = NodeIdentity {
        label: sanitize_node_label(&hostname),
        hostname,
        role: NodeRole::default_for_profile(install_profile),
        install_profile,
    };
    persist_node_identity(&identity)?;
    sync_runtime_config(&identity)?;
    Ok(identity)
}
