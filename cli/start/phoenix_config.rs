//! Phoenix.toml generation for subsystem configuration.

use std::path::Path;

pub(crate) fn write_phoenix_config(
    config_path: &Path,
    runtime_bin: &str,
    boot_file: &Path,
    source_dir: &Path,
    system_dir: &Path,
    vault_path: &Path,
    wallet_db_path: &Path,
    lib_dir: &Path,
    env: &str,
    log_level: &str,
    node_identity: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    let sbcl_command = format!(
        "sbcl --noinform --disable-debugger --load {} --eval '(harmonia:start)'",
        boot_file.display()
    );

    let env_toml = format!(
        r#"HARMONIA_STATE_ROOT = "{state_root}"
HARMONIA_SYSTEM_DIR = "{state_root}"
HARMONIA_VAULT_DB = "{vault_db}"
HARMONIA_VAULT_WALLET_DB = "{wallet_db}"
HARMONIA_LIB_DIR = "{lib_dir}"
HARMONIA_SOURCE_DIR = "{source_dir}"
HARMONIA_NODE_LABEL = "{node_label}"
HARMONIA_NODE_ROLE = "{node_role}"
HARMONIA_LOG_LEVEL = "{log_level}"
HARMONIA_ENV = "{env}""#,
        state_root = system_dir.display(),
        vault_db = vault_path.display(),
        wallet_db = wallet_db_path.display(),
        lib_dir = lib_dir.display(),
        source_dir = source_dir.display(),
        node_label = node_identity.label,
        node_role = node_identity.role.as_str(),
    );

    let config = format!(
        r#"[phoenix]
health_port = 9100
shutdown_timeout_secs = 5

[[subsystem]]
name = "harmonia-runtime"
command = "{runtime_bin}"
restart_policy = "always"
max_restarts = 10
backoff_base_ms = 500
backoff_max_ms = 60000
core = true

[subsystem.env]
{env_toml}

[[subsystem]]
name = "sbcl-agent"
command = "{sbcl_command}"
restart_policy = "always"
max_restarts = 10
backoff_base_ms = 2000
backoff_max_ms = 120000
startup_delay_ms = 2000
core = true

[subsystem.env]
{env_toml}
"#
    );

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(config_path, config)?;
    Ok(())
}
