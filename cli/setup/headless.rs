//! Headless provisioning from a JSON config file.

use std::fs;
use std::path::{Path, PathBuf};

use super::resolve_configured_modules;

/// Headless setup: provision vault secrets and config-store values from a JSON file.
///
/// Called by install.sh --config or `harmonia setup --headless-config config.json`.
/// The JSON schema is documented in config/install-config.template.json.
pub fn run_headless(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let raw = fs::read_to_string(config_path)
        .map_err(|e| format!("cannot read config file '{}': {}", config_path, e))?;
    let config: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("invalid JSON in '{}': {}", config_path, e))?;

    eprintln!("[INFO] [setup] Headless provisioning from {}", config_path);

    // Initialize system directory
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = if let Some(p) = config.pointer("/paths/data_dir").and_then(|v| v.as_str()) {
        PathBuf::from(p)
    } else {
        home.join(".harmoniis").join("harmonia")
    };
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;

    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());

    // Initialize vault
    let vault_path = system_dir.join("vault.db");
    std::env::set_var("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref());

    // Check for wallet
    let wallet_path = if let Some(p) = config.pointer("/paths/wallet_db").and_then(|v| v.as_str()) {
        PathBuf::from(p)
    } else {
        home.join(".harmoniis").join("wallet").join("master.db")
    };
    if wallet_path.exists() {
        std::env::set_var(
            "HARMONIA_WALLET_ROOT",
            wallet_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_string_lossy()
                .as_ref(),
        );
        std::env::set_var(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_path.to_string_lossy().as_ref(),
        );
    }

    harmonia_vault::init_from_env().map_err(|e| format!("vault init failed: {e}"))?;
    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    // Write system paths
    let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
    };
    cs("global", "state-root", &system_dir.to_string_lossy())?;
    cs("global", "system-dir", &system_dir.to_string_lossy())?;

    // Provision vault secrets
    if let Some(secrets) = config.get("vault_secrets").and_then(|v| v.as_object()) {
        for (symbol, value) in secrets {
            if let Some(val) = value.as_str() {
                if !val.is_empty() {
                    harmonia_vault::set_secret_for_symbol(symbol, val)
                        .map_err(|e| format!("vault write failed for {}: {e}", symbol))?;
                    eprintln!("[INFO] [setup]   vault: {}", symbol);
                }
            }
        }
    }

    // Provision config-store keys
    if let Some(config_values) = config.get("config_store").and_then(|v| v.as_object()) {
        for (component, scopes) in config_values {
            if let Some(scopes_obj) = scopes.as_object() {
                for (scope, keys) in scopes_obj {
                    if let Some(keys_obj) = keys.as_object() {
                        for (key, value) in keys_obj {
                            if let Some(val) = value.as_str() {
                                harmonia_config_store::set_config(component, scope, key, val)
                                    .map_err(|e| format!("config-store write failed: {e}"))?;
                                eprintln!(
                                    "[INFO] [setup]   config: {}/{}/{}",
                                    component, scope, key
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Provision workspace path
    if let Some(workspace) = config.pointer("/paths/workspace").and_then(|v| v.as_str()) {
        let workspace_path = PathBuf::from(workspace);
        fs::create_dir_all(&workspace_path)?;
        let workspace_config = format!(
            "(:workspace\n  (:system-dir \"{}\")\n  (:user-workspace \"{}\"))\n",
            system_dir.display(),
            workspace_path.display()
        );
        fs::write(
            system_dir.join("config").join("workspace.sexp"),
            &workspace_config,
        )?;
        eprintln!("[INFO] [setup]   workspace: {}", workspace);
    }

    // Auto-detect and persist enabled runtime modules
    let enabled_modules = resolve_configured_modules();
    if !enabled_modules.is_empty() {
        let csv = enabled_modules.join(",");
        harmonia_config_store::set_config("harmonia-cli", "runtime", "components", &csv)
            .map_err(|e| format!("failed to persist runtime components: {e}"))?;
        eprintln!("[INFO] [setup]   runtime modules: {}", csv);
    }

    eprintln!("[INFO] [setup] Headless provisioning complete");
    Ok(())
}
