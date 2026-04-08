//! Daemon/process spawning: broker, node-service, phoenix (foreground/background).

use console::style;
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub(crate) fn should_start_embedded_broker() -> bool {
    harmonia_config_store::get_config("harmonia-cli", "mqtt-broker", "mode")
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty() && !v.eq_ignore_ascii_case("external"))
        .unwrap_or(false)
}

pub(crate) fn spawn_broker_process(
    source_dir: &Path,
    system_dir: &Path,
    vault_path: &Path,
    wallet_db_path: &Path,
    lib_dir: &Path,
) -> Result<Child, Box<dyn std::error::Error>> {
    let pid_path = crate::paths::broker_pid_path()?;
    if pid_path.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                #[cfg(unix)]
                if unsafe { libc::kill(pid, 0) } == 0 {
                    return Err(format!("embedded MQTT broker already running (PID {pid})").into());
                }
            }
        }
        let _ = std::fs::remove_file(&pid_path);
    }

    let log_path = crate::paths::broker_log_path()?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err_file = log_file.try_clone()?;
    let exe = std::env::current_exe()?;

    let child = Command::new(exe)
        .arg("broker")
        .env("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_SYSTEM_DIR", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref())
        .env(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db_path.to_string_lossy().as_ref(),
        )
        .env("HARMONIA_LIB_DIR", lib_dir.to_string_lossy().as_ref())
        .current_dir(source_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()?;

    std::fs::write(&pid_path, child.id().to_string())?;
    Ok(child)
}

pub(crate) fn should_start_node_service(node: &crate::paths::NodeIdentity) -> bool {
    node.role == crate::paths::NodeRole::Agent
}

pub(crate) fn spawn_node_service_process(
    source_dir: &Path,
    system_dir: &Path,
    vault_path: &Path,
    wallet_db_path: &Path,
    lib_dir: &Path,
    node_identity: &crate::paths::NodeIdentity,
) -> Result<Child, Box<dyn std::error::Error>> {
    let pid_path = crate::paths::node_service_pid_path()?;
    if pid_path.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                #[cfg(unix)]
                if unsafe { libc::kill(pid, 0) } == 0 {
                    return Err(format!("node-service already running (PID {pid})").into());
                }
            }
        }
        let _ = std::fs::remove_file(&pid_path);
    }

    let log_path = crate::paths::node_service_log_path()?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let err_file = log_file.try_clone()?;
    let exe = std::env::current_exe()?;

    let child = Command::new(exe)
        .arg("node-service")
        .env("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_SYSTEM_DIR", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref())
        .env(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db_path.to_string_lossy().as_ref(),
        )
        .env("HARMONIA_LIB_DIR", lib_dir.to_string_lossy().as_ref())
        .env("HARMONIA_NODE_LABEL", &node_identity.label)
        .env("HARMONIA_NODE_ROLE", node_identity.role.as_str())
        .env(
            "HARMONIA_INSTALL_PROFILE",
            node_identity.install_profile.as_str(),
        )
        .current_dir(source_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(err_file))
        .spawn()?;

    std::fs::write(&pid_path, child.id().to_string())?;
    Ok(child)
}

/// Print helpful post-start messages for daemon mode.
pub(crate) fn print_daemon_info(
    pid: u32,
    log_path: &Path,
    pid_path: &Path,
    node_identity: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{} Harmonia started (PID {})",
        style("✓").green().bold(),
        pid
    );
    println!("  log: {}", log_path.display());
    println!("  pid: {}", pid_path.display());
    println!();
    println!("  {}   to open a session", style("harmonia").cyan().bold());
    println!("  {}   to stop", style("harmonia stop").cyan().bold());
    println!(
        "  {} to view logs",
        style(format!("tail -f {}", log_path.display()))
            .cyan()
            .bold()
    );
    if should_start_embedded_broker() {
        println!("  broker: {}", crate::paths::broker_log_path()?.display());
    }
    if should_start_node_service(node_identity) {
        println!(
            "  node-service: {}",
            crate::paths::node_service_log_path()?.display()
        );
    }
    Ok(())
}
