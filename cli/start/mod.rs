//! `harmonia start` — validate environment, build artifacts, launch daemon.

mod build;
mod daemon;
mod phoenix_config;
mod validation;

use console::style;
use std::process::{Child, Command, Stdio};

pub fn run(env: &str, foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let log_level = crate::paths::config_value("global", "log-level")
        .ok()
        .flatten()
        .unwrap_or_else(|| "info".to_string());

    match env {
        "test" | "dev" | "prod" => {}
        _ => return Err(format!("invalid environment: {} (use test, dev, or prod)", env).into()),
    }

    let system_dir = crate::paths::data_dir()?;
    let node_identity = crate::paths::current_node_identity()?;
    if !system_dir.join("vault.db").exists() {
        println!("{} Harmonia is not set up yet. Run:", style("!").red().bold());
        println!("  {}", style("harmonia setup").cyan().bold());
        return Err("run `harmonia setup` first".into());
    }

    // Check if already running
    let pid_path = crate::paths::pid_path()?;
    if pid_path.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                #[cfg(unix)]
                {
                    let alive = unsafe { libc::kill(pid, 0) } == 0;
                    if alive {
                        eprintln!("{} Harmonia is already running (PID {}). Use:", style("!").yellow().bold(), pid);
                        eprintln!("  {}  to open the current session", style("harmonia").cyan().bold());
                        eprintln!("  {}  to restart", style("harmonia stop && harmonia start").cyan().bold());
                        return Ok(());
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&pid_path);
    }

    // Bootstrap env
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());
    std::env::set_var("HARMONIA_NODE_LABEL", &node_identity.label);
    std::env::set_var("HARMONIA_NODE_ROLE", node_identity.role.as_str());
    let _ = harmonia_config_store::init_v2();

    let source_dir = validation::resolve_source_dir(&system_dir)?;
    let boot_file = source_dir.join("src").join("core").join("boot.lisp");
    if !boot_file.exists() {
        return Err(format!("boot.lisp not found at {}", boot_file.display()).into());
    }

    if !validation::check_command("sbcl") {
        println!("{} SBCL not found. Install it first.", style("!").red().bold());
        println!("  macOS:   brew install sbcl");
        println!("  Ubuntu:  sudo apt install sbcl");
        println!("  FreeBSD: sudo pkg install sbcl");
        return Err("SBCL is required".into());
    }

    let lib_dir = validation::resolve_lib_dir(&source_dir);
    build::ensure_runtime_artifacts(&source_dir, &lib_dir)?;

    let vault_path = system_dir.join("vault.db");
    let wallet_root = crate::paths::wallet_root_path()?;
    let wallet_db_path = crate::paths::wallet_db_path()?;

    // Write resolved paths to config-store
    for (scope, key, val) in [
        ("global", "source-dir", source_dir.to_string_lossy().to_string()),
        ("global", "lib-dir", lib_dir.to_string_lossy().to_string()),
        ("global", "wallet-root", wallet_root.to_string_lossy().to_string()),
        ("global", "wallet-db", wallet_db_path.to_string_lossy().to_string()),
        ("global", "system-dir", system_dir.to_string_lossy().to_string()),
        // Ensure state-root is always set so signalograd (and other components)
        // checkpoint to the data directory, not the source tree.
        ("global", "state-root", system_dir.to_string_lossy().to_string()),
    ] {
        let _ = harmonia_config_store::set_config("harmonia-cli", scope, key, &val);
    }
    if let Ok(share) = crate::paths::share_dir() {
        let _ = harmonia_config_store::set_config("harmonia-cli", "global", "share-dir", &share.to_string_lossy());
    }
    let _ = harmonia_config_store::set_config("harmonia-cli", "global", "env", env);

    println!("{} Starting Harmonia (env={})", style("->").cyan().bold(), style(env).green());
    println!("  source:    {}", source_dir.display());
    println!("  libraries: {}", lib_dir.display());
    println!("  vault:     {}", vault_path.display());
    println!("  wallet:    {}", wallet_db_path.display());
    println!("  config:    {}", system_dir.join("config.db").display());
    println!("  workspace: {}", system_dir.display());
    println!("  node:      {} ({})", node_identity.label, node_identity.role.as_str());
    println!();

    let mut broker_child = if daemon::should_start_embedded_broker() {
        Some(daemon::spawn_broker_process(&source_dir, &system_dir, &vault_path, &wallet_db_path, &lib_dir)?)
    } else { None };
    let mut node_service_child: Option<Child> = None;

    // Generate phoenix.toml
    let phoenix_config_path = system_dir.join("phoenix.toml");
    let phoenix_bin = validation::find_phoenix_binary(&source_dir)?;
    let runtime_bin = validation::find_sibling_binary(&phoenix_bin, "harmonia-runtime");
    phoenix_config::write_phoenix_config(&phoenix_config_path, &runtime_bin, &boot_file, &source_dir, &system_dir, &vault_path, &wallet_db_path, &lib_dir, env, &log_level, &node_identity)?;

    let env_vars: Vec<(&str, String)> = vec![
        ("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().into()),
        ("HARMONIA_SYSTEM_DIR", system_dir.to_string_lossy().into()),
        ("HARMONIA_VAULT_DB", vault_path.to_string_lossy().into()),
        ("HARMONIA_VAULT_WALLET_DB", wallet_db_path.to_string_lossy().into()),
        ("HARMONIA_LIB_DIR", lib_dir.to_string_lossy().into()),
        ("HARMONIA_SOURCE_DIR", source_dir.to_string_lossy().into()),
        ("HARMONIA_NODE_LABEL", node_identity.label.clone()),
        ("HARMONIA_NODE_ROLE", node_identity.role.as_str().to_string()),
        ("HARMONIA_LOG_LEVEL", log_level.clone()),
        ("HARMONIA_ENV", env.to_string()),
        ("PHOENIX_CONFIG_PATH", phoenix_config_path.to_string_lossy().into()),
    ];

    if foreground {
        if daemon::should_start_node_service(&node_identity) {
            match daemon::spawn_node_service_process(&source_dir, &system_dir, &vault_path, &wallet_db_path, &lib_dir, &node_identity) {
                Ok(child) => node_service_child = Some(child),
                Err(e) => println!("  {} node-service: {}", console::style("!").yellow().bold(), e),
            }
        }
        let mut cmd = Command::new(&phoenix_bin);
        for (k, v) in &env_vars { cmd.env(k, v); }
        let status = cmd.current_dir(&source_dir).status()?;
        if let Some(child) = broker_child.as_mut() { let _ = child.kill(); let _ = child.wait(); let _ = std::fs::remove_file(crate::paths::broker_pid_path()?); }
        if let Some(child) = node_service_child.as_mut() { let _ = child.kill(); let _ = child.wait(); let _ = std::fs::remove_file(crate::paths::node_service_pid_path()?); }
        if !status.success() { return Err("Phoenix exited with error".into()); }
    } else {
        let log_path = crate::paths::log_path()?;
        let log_file = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
        let err_file = log_file.try_clone()?;
        let mut cmd = Command::new(&phoenix_bin);
        for (k, v) in &env_vars { cmd.env(k, v); }
        let child = cmd.current_dir(&source_dir).stdin(Stdio::null()).stdout(Stdio::from(log_file)).stderr(Stdio::from(err_file)).spawn()?;
        let pid = child.id();
        std::fs::write(&pid_path, pid.to_string())?;

        if daemon::should_start_node_service(&node_identity) {
            match daemon::spawn_node_service_process(&source_dir, &system_dir, &vault_path, &wallet_db_path, &lib_dir, &node_identity) {
                Ok(_) => {}
                Err(e) => println!("  {} node-service: {}", console::style("!").yellow().bold(), e),
            }
        }
        daemon::print_daemon_info(pid, &log_path, &pid_path, &node_identity)?;
    }

    Ok(())
}
