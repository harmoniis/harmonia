//! Shared helpers for node_rpc: path resolution, vault env, config access, time.

use harmonia_node_rpc::{NodePathRef, NodePathScope};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn resolve_path(
    node: &crate::paths::NodeIdentity,
    reference: &NodePathRef,
) -> Result<PathBuf, String> {
    match reference.scope {
        NodePathScope::Absolute => {
            let path = PathBuf::from(&reference.path);
            if !path.is_absolute() {
                return Err("absolute scope requires an absolute path".to_string());
            }
            Ok(path)
        }
        NodePathScope::Workspace => resolve_relative_in_root(
            &crate::paths::user_workspace().map_err(|e| e.to_string())?,
            &reference.path,
        ),
        NodePathScope::Home => {
            let home =
                dirs::home_dir().ok_or_else(|| "cannot determine home directory".to_string())?;
            resolve_relative_in_root(&home, &reference.path)
        }
        NodePathScope::Data => resolve_relative_in_root(
            &crate::paths::data_dir().map_err(|e| e.to_string())?,
            &reference.path,
        ),
        NodePathScope::Node => resolve_relative_in_root(
            &crate::paths::node_dir(&node.label).map_err(|e| e.to_string())?,
            &reference.path,
        ),
    }
}

pub(crate) fn resolve_relative_in_root(root: &Path, raw: &str) -> Result<PathBuf, String> {
    let input = Path::new(raw);
    if input.is_absolute() {
        return Err("scoped paths must be relative".to_string());
    }
    for component in input.components() {
        if matches!(component, Component::ParentDir) {
            return Err("path traversal rejected".to_string());
        }
    }
    Ok(root.join(input))
}

pub(crate) fn default_exec_cwd(node: &crate::paths::NodeIdentity) -> Result<PathBuf, String> {
    crate::paths::user_workspace()
        .map_err(|_| ())
        .or_else(|_| crate::paths::node_dir(&node.label).map_err(|_| ()))
        .map_err(|_| "no default working directory available".to_string())
}

pub(crate) fn bind_vault_env() -> Result<(), String> {
    let vault_db = crate::paths::vault_db_path().map_err(|e| e.to_string())?;
    let wallet_root = crate::paths::wallet_root_path().map_err(|e| e.to_string())?;
    let wallet_db = crate::paths::wallet_db_path().map_err(|e| e.to_string())?;
    let state_root = crate::paths::data_dir().map_err(|e| e.to_string())?;
    let _ = crate::paths::set_config_value("global", "vault-db", &vault_db.to_string_lossy());
    let _ =
        crate::paths::set_config_value("global", "wallet-root", &wallet_root.to_string_lossy());
    let _ = crate::paths::set_config_value("global", "wallet-db", &wallet_db.to_string_lossy());
    let _ =
        crate::paths::set_config_value("global", "state-root", &state_root.to_string_lossy());
    std::env::set_var("HARMONIA_VAULT_DB", vault_db.to_string_lossy().as_ref());
    std::env::set_var(
        "HARMONIA_WALLET_ROOT",
        wallet_root.to_string_lossy().as_ref(),
    );
    std::env::set_var(
        "HARMONIA_VAULT_WALLET_DB",
        wallet_db.to_string_lossy().as_ref(),
    );
    std::env::set_var("HARMONIA_STATE_ROOT", state_root.to_string_lossy().as_ref());
    Ok(())
}

pub(crate) fn config_has(component: &str, key: &str) -> bool {
    harmonia_config_store::get_own(component, key)
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn vault_has(component: &str, symbols: &[&str]) -> bool {
    for sym in symbols {
        if let Ok(Some(v)) = harmonia_vault::get_secret_for_component(component, sym) {
            if !v.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

pub(crate) fn vault_get(component: &str, symbols: &[&str]) -> Option<String> {
    for sym in symbols {
        if let Ok(Some(v)) = harmonia_vault::get_secret_for_component(component, sym) {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }
    None
}

pub(crate) fn config_get(component: &str, key: &str) -> Option<String> {
    harmonia_config_store::get_own(component, key)
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
}

pub(crate) fn config_set(component: &str, key: &str, value: &str) -> Result<(), String> {
    harmonia_config_store::set_config(component, component, key, value)
}
