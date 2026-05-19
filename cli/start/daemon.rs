//! Daemon/process spawning: broker, node-service, phoenix (foreground/background).

use console::style;
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// Reap orphaned phoenix children — `harmonia-runtime`, the SBCL agent
/// loaded with our `boot.lisp`, and `provision-server` — when the supervisor
/// has died but its children are still alive. Without this, a fresh
/// `harmonia start` would spawn duplicates while the orphans hold sockets,
/// IPC files, and database connections from the previous incarnation.
pub(crate) fn reap_orphan_children(boot_file_path: &Path) {
    let needles: [&str; 3] = ["harmonia-runtime", "provision-server", ""];
    let boot_str = boot_file_path.to_string_lossy().into_owned();

    let output = match Command::new("ps").args(["-A", "-o", "pid=,args="]).output() {
        Ok(o) => o,
        Err(_) => return,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let me = std::process::id() as i32;

    for line in stdout.lines() {
        let line = line.trim_start();
        let (pid_part, rest) = match line.split_once(' ') {
            Some(p) => p,
            None => continue,
        };
        let Ok(pid) = pid_part.parse::<i32>() else {
            continue;
        };
        if pid == me {
            continue;
        }

        let matches_runtime = rest.contains(needles[0]);
        let matches_provision = rest.contains(needles[1]);
        let matches_sbcl_agent =
            rest.contains("sbcl") && !boot_str.is_empty() && rest.contains(&boot_str);

        if matches_runtime || matches_provision || matches_sbcl_agent {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            eprintln!(
                "{} reaped orphan child PID {} ({})",
                console::style("!").yellow().bold(),
                pid,
                rest.split_whitespace().next().unwrap_or("?")
            );
        }
    }

    // Brief grace period for SIGTERM, then SIGKILL stragglers.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let output = match Command::new("ps").args(["-A", "-o", "pid=,args="]).output() {
        Ok(o) => o,
        Err(_) => return,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let line = line.trim_start();
        let (pid_part, rest) = match line.split_once(' ') {
            Some(p) => p,
            None => continue,
        };
        let Ok(pid) = pid_part.parse::<i32>() else {
            continue;
        };
        if pid == me {
            continue;
        }
        let matches_runtime = rest.contains(needles[0]);
        let matches_provision = rest.contains(needles[1]);
        let matches_sbcl_agent =
            rest.contains("sbcl") && !boot_str.is_empty() && rest.contains(&boot_str);
        if matches_runtime || matches_provision || matches_sbcl_agent {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

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
