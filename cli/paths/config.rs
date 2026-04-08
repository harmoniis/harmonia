use std::path::{Path, PathBuf};

pub(super) fn ensure_state_root_env() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = super::data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", dir.to_string_lossy().as_ref());
    }
    Ok(dir)
}

pub fn config_value(scope: &str, key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let _ = ensure_state_root_env()?;
    harmonia_config_store::init_v2()?;
    Ok(harmonia_config_store::get_config(
        "harmonia-cli",
        scope,
        key,
    )?)
}

pub fn set_config_value(
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = ensure_state_root_env()?;
    harmonia_config_store::init_v2()?;
    harmonia_config_store::set_config("harmonia-cli", scope, key, value)?;
    Ok(())
}

/// Read the user workspace path from config/workspace.sexp
pub fn user_workspace() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = super::data_dir()?.join("config").join("workspace.sexp");
    let content = std::fs::read_to_string(&config)?;
    if let Some(start) = content.find(":user-workspace") {
        let rest = &content[start..];
        if let Some(q1) = rest.find('"') {
            if let Some(q2) = rest[q1 + 1..].find('"') {
                return Ok(PathBuf::from(&rest[q1 + 1..q1 + 1 + q2]));
            }
        }
    }
    Err("workspace path not configured — run `harmonia setup`".into())
}

fn canonical_wallet_root(home: &Path) -> PathBuf {
    home.join(".harmoniis").join("wallet")
}

fn wallet_root_from_master_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "master.db" | "rgb.db" | "wallet.db" | "webcash.db" | "bitcoin.db"
            )
        })
        .unwrap_or(false)
    {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    }
}

fn configured_wallet_root() -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("HARMONIA_WALLET_ROOT") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(PathBuf::from(trimmed)));
        }
    }
    if let Ok(path) = std::env::var("HARMONIA_VAULT_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(path) = std::env::var("HARMONIA_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(path) = std::env::var("HARMONIIS_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(Some(path)) = config_value("global", "wallet-root") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(PathBuf::from(trimmed)));
        }
    }
    if let Ok(Some(path)) = config_value("global", "wallet-db") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    Ok(None)
}

pub fn wallet_root_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    if let Some(root) = configured_wallet_root()? {
        return Ok(root);
    }
    Ok(canonical_wallet_root(&home))
}

/// The master wallet DB path.
pub fn wallet_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(wallet_root_path()?.join("master.db"))
}

pub fn vault_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(super::data_dir()?.join("vault.db"))
}
