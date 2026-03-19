use console::style;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub fn run(env: &str, foreground: bool) -> Result<(), Box<dyn std::error::Error>> {
    let log_level = crate::paths::config_value("global", "log-level")
        .ok()
        .flatten()
        .unwrap_or_else(|| "info".to_string());
    // Validate environment
    match env {
        "test" | "dev" | "prod" => {}
        _ => return Err(format!("invalid environment: {} (use test, dev, or prod)", env).into()),
    }

    // Check system workspace exists
    let system_dir = crate::paths::data_dir()?;
    let node_identity = crate::paths::current_node_identity()?;
    if !system_dir.join("vault.db").exists() {
        println!(
            "{} Harmonia is not set up yet. Run:",
            style("!").red().bold()
        );
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
                        eprintln!(
                            "{} Harmonia is already running (PID {}). Use:",
                            style("!").yellow().bold(),
                            pid
                        );
                        eprintln!(
                            "  {}  to open the current session",
                            style("harmonia").cyan().bold()
                        );
                        eprintln!(
                            "  {}  to restart",
                            style("harmonia stop && harmonia start").cyan().bold()
                        );
                        return Ok(());
                    }
                }
            }
        }
        // Stale PID file
        let _ = std::fs::remove_file(&pid_path);
    }

    // Bootstrap: set STATE_ROOT so config-store/vault can find their DBs
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());
    std::env::set_var("HARMONIA_NODE_LABEL", &node_identity.label);
    std::env::set_var("HARMONIA_NODE_ROLE", node_identity.role.as_str());

    // Initialize config-store to read stored paths
    let _ = harmonia_config_store::init_v2();

    // Resolve paths: prefer config-store, fallback to auto-detection
    let source_dir = resolve_source_dir(&system_dir)?;
    let boot_file = source_dir.join("src").join("core").join("boot.lisp");
    if !boot_file.exists() {
        return Err(format!("boot.lisp not found at {}", boot_file.display()).into());
    }

    // Check SBCL
    if !check_command("sbcl") {
        println!(
            "{} SBCL not found. Install it first.",
            style("!").red().bold()
        );
        println!("  macOS:   brew install sbcl");
        println!("  Ubuntu:  sudo apt install sbcl");
        println!("  FreeBSD: sudo pkg install sbcl");
        return Err("SBCL is required".into());
    }

    let lib_dir = resolve_lib_dir(&source_dir);

    // Ensure native runtime artifacts exist before booting Lisp.
    ensure_runtime_artifacts(&source_dir, &lib_dir)?;

    // Vault paths
    let vault_path = system_dir.join("vault.db");
    let wallet_root = crate::paths::wallet_root_path()?;
    let wallet_db_path = crate::paths::wallet_db_path()?;

    // Write resolved paths back to config-store for runtime access
    let _ = harmonia_config_store::set_config(
        "harmonia-cli",
        "global",
        "source-dir",
        &source_dir.to_string_lossy(),
    );
    let _ = harmonia_config_store::set_config(
        "harmonia-cli",
        "global",
        "lib-dir",
        &lib_dir.to_string_lossy(),
    );
    let _ = harmonia_config_store::set_config(
        "harmonia-cli",
        "global",
        "wallet-root",
        &wallet_root.to_string_lossy(),
    );
    let _ = harmonia_config_store::set_config(
        "harmonia-cli",
        "global",
        "wallet-db",
        &wallet_db_path.to_string_lossy(),
    );
    let _ = harmonia_config_store::set_config(
        "harmonia-cli",
        "global",
        "system-dir",
        &system_dir.to_string_lossy(),
    );
    if let Ok(share) = crate::paths::share_dir() {
        let _ = harmonia_config_store::set_config(
            "harmonia-cli",
            "global",
            "share-dir",
            &share.to_string_lossy(),
        );
    }
    let _ = harmonia_config_store::set_config("harmonia-cli", "global", "env", env);

    println!(
        "{} Starting Harmonia (env={})",
        style("→").cyan().bold(),
        style(env).green()
    );
    println!("  source:    {}", source_dir.display());
    println!("  libraries: {}", lib_dir.display());
    println!("  vault:     {}", vault_path.display());
    println!("  wallet:    {}", wallet_db_path.display());
    println!("  config:    {}", system_dir.join("config.db").display());
    println!("  workspace: {}", system_dir.display());
    println!(
        "  node:      {} ({})",
        node_identity.label,
        node_identity.role.as_str()
    );
    println!();

    let mut broker_child = if should_start_embedded_broker() {
        Some(spawn_broker_process(
            &source_dir,
            &system_dir,
            &vault_path,
            &wallet_db_path,
            &lib_dir,
        )?)
    } else {
        None
    };
    let mut node_service_child: Option<Child> = None;

    // Generate phoenix.toml with correct paths for this installation
    let phoenix_config_path = system_dir.join("phoenix.toml");
    let phoenix_bin = find_phoenix_binary(&source_dir)?;
    let runtime_bin = find_sibling_binary(&phoenix_bin, "harmonia-runtime");
    write_phoenix_config(&phoenix_config_path, &runtime_bin, &boot_file, &source_dir)?;

    // Common env vars inherited by all Phoenix children
    let env_vars: Vec<(&str, String)> = vec![
        ("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().into()),
        ("HARMONIA_SYSTEM_DIR", system_dir.to_string_lossy().into()),
        ("HARMONIA_VAULT_DB", vault_path.to_string_lossy().into()),
        (
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db_path.to_string_lossy().into(),
        ),
        ("HARMONIA_LIB_DIR", lib_dir.to_string_lossy().into()),
        ("HARMONIA_SOURCE_DIR", source_dir.to_string_lossy().into()),
        ("HARMONIA_NODE_LABEL", node_identity.label.clone()),
        (
            "HARMONIA_NODE_ROLE",
            node_identity.role.as_str().to_string(),
        ),
        ("HARMONIA_LOG_LEVEL", log_level.clone()),
        ("HARMONIA_ENV", env.to_string()),
        (
            "PHOENIX_CONFIG_PATH",
            phoenix_config_path.to_string_lossy().into(),
        ),
    ];

    if foreground {
        // Foreground mode — block until Phoenix exits
        if should_start_node_service(&node_identity) {
            match spawn_node_service_process(
                &source_dir,
                &system_dir,
                &vault_path,
                &wallet_db_path,
                &lib_dir,
                &node_identity,
            ) {
                Ok(child) => node_service_child = Some(child),
                Err(e) => {
                    println!(
                        "  {} node-service: {}",
                        console::style("!").yellow().bold(),
                        e
                    );
                }
            }
        }

        let mut cmd = Command::new(&phoenix_bin);
        for (k, v) in &env_vars {
            cmd.env(k, v);
        }
        let status = cmd.current_dir(&source_dir).status()?;

        if let Some(child) = broker_child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(crate::paths::broker_pid_path()?);
        }
        if let Some(child) = node_service_child.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(crate::paths::node_service_pid_path()?);
        }

        if !status.success() {
            return Err("Phoenix exited with error".into());
        }
    } else {
        // Daemon mode — spawn Phoenix in background
        let log_path = crate::paths::log_path()?;
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let err_file = log_file.try_clone()?;

        let mut cmd = Command::new(&phoenix_bin);
        for (k, v) in &env_vars {
            cmd.env(k, v);
        }
        let child = cmd
            .current_dir(&source_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .spawn()?;

        let pid = child.id();
        std::fs::write(&pid_path, pid.to_string())?;

        if should_start_node_service(&node_identity) {
            match spawn_node_service_process(
                &source_dir,
                &system_dir,
                &vault_path,
                &wallet_db_path,
                &lib_dir,
                &node_identity,
            ) {
                Ok(_) => {}
                Err(e) => {
                    // Non-fatal: node-service may already be running from a prior start.
                    // The daemon itself is already spawned — don't abort.
                    println!(
                        "  {} node-service: {}",
                        console::style("!").yellow().bold(),
                        e
                    );
                }
            }
        }

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
        if should_start_node_service(&node_identity) {
            println!(
                "  node-service: {}",
                crate::paths::node_service_log_path()?.display()
            );
        }
    }

    Ok(())
}

fn check_command(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn should_start_embedded_broker() -> bool {
    harmonia_config_store::get_config("harmonia-cli", "mqtt-broker", "mode")
        .ok()
        .flatten()
        .map(|v| !v.trim().is_empty() && !v.eq_ignore_ascii_case("external"))
        .unwrap_or(false)
}

fn spawn_broker_process(
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

fn should_start_node_service(node: &crate::paths::NodeIdentity) -> bool {
    node.role == crate::paths::NodeRole::Agent
}

fn spawn_node_service_process(
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

fn is_runtime_root(path: &Path) -> bool {
    path.join("src").join("core").join("boot.lisp").exists()
}

fn is_installed_binary() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let installed_bin_dir = home.join(".local").join("bin");
    std::env::current_exe()
        .ok()
        .map(|exe| exe.starts_with(installed_bin_dir))
        .unwrap_or(false)
}

fn is_truthy_config(raw: &str) -> bool {
    let value = raw.trim();
    !value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "nil" | "no" | "off"
        )
}

fn source_rewrite_enabled() -> bool {
    harmonia_config_store::get_config("harmonia-cli", "evolution", "source-rewrite-enabled")
        .ok()
        .flatten()
        .map(|raw| is_truthy_config(&raw))
        .unwrap_or(false)
}

fn required_runtime_libraries() -> Vec<String> {
    // No shared libraries required — all Rust code is compiled into
    // the harmonia-runtime binary. This function returns an empty list.
    Vec::new()
}

fn resolve_lib_dir(source_dir: &Path) -> PathBuf {
    // Check config-store first
    if let Ok(Some(stored)) = harmonia_config_store::get_config("harmonia-cli", "global", "lib-dir")
    {
        let p = PathBuf::from(&stored);
        if p.exists() {
            return p;
        }
    }
    // Platform-standard lib dir (~/.local/lib/harmonia/)
    if let Ok(platform_lib) = crate::paths::lib_dir() {
        if platform_lib.exists()
            && platform_lib
                .read_dir()
                .map_or(false, |mut d| d.next().is_some())
        {
            return platform_lib;
        }
    }
    // Dev mode: target/release/ in source tree
    let candidate_target = source_dir.join("target").join("release");
    if candidate_target.exists() {
        return candidate_target;
    }
    candidate_target
}

fn missing_runtime_artifacts(lib_dir: &Path) -> Vec<String> {
    required_runtime_libraries()
        .into_iter()
        .filter(|name| !lib_dir.join(name).exists())
        .collect()
}

fn ensure_runtime_artifacts(
    source_dir: &Path,
    lib_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let missing = missing_runtime_artifacts(lib_dir);
    if missing.is_empty() {
        return Ok(());
    }

    let has_source_build = source_dir.join("Cargo.toml").exists();
    if !has_source_build {
        return Err(format!(
            "missing runtime libraries in {}: {}",
            lib_dir.display(),
            missing.join(", ")
        )
        .into());
    }

    if !check_command("cargo") {
        return Err("cargo is required to build missing runtime artifacts".into());
    }

    println!(
        "{} Missing runtime libraries ({}). Building release workspace...",
        style("→").cyan().bold(),
        missing.len()
    );
    let status = Command::new("cargo")
        .args(["build", "--workspace", "--release"])
        .current_dir(source_dir)
        .status()?;
    if !status.success() {
        return Err("failed to build release runtime artifacts".into());
    }

    let rebuilt_lib_dir = resolve_lib_dir(source_dir);
    let after = missing_runtime_artifacts(&rebuilt_lib_dir);
    if !after.is_empty() {
        return Err(format!(
            "runtime libraries still missing after build: {}",
            after.join(", ")
        )
        .into());
    }
    Ok(())
}

fn find_phoenix_binary(source_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // 1. Same directory as the running harmonia binary
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.with_file_name("harmonia-phoenix");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    // 2. In PATH
    if check_command("harmonia-phoenix") {
        return Ok(PathBuf::from("harmonia-phoenix"));
    }
    // 3. Dev mode: target/release/phoenix
    let dev = source_dir.join("target").join("release").join("phoenix");
    if dev.exists() {
        return Ok(dev);
    }
    Err("harmonia-phoenix binary not found — run install script".into())
}

fn find_sibling_binary(phoenix_bin: &Path, name: &str) -> String {
    // Try sibling of phoenix binary first
    if let Some(dir) = phoenix_bin.parent() {
        let sibling = dir.join(name);
        if sibling.exists() {
            return sibling.to_string_lossy().into();
        }
    }
    // Fallback: assume it's in PATH
    name.to_string()
}

fn write_phoenix_config(
    config_path: &Path,
    runtime_bin: &str,
    boot_file: &Path,
    _source_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let sbcl_command = format!(
        "sbcl --noinform --disable-debugger --load {} --eval '(harmonia:start)'",
        boot_file.display()
    );
    let config = format!(
        r#"[phoenix]
health_port = 9100
shutdown_timeout_secs = 30

[[subsystem]]
name = "harmonia-runtime"
command = "{runtime_bin}"
restart_policy = "always"
max_restarts = 10
backoff_base_ms = 500
backoff_max_ms = 60000
core = true

[[subsystem]]
name = "sbcl-agent"
command = "{sbcl_command}"
restart_policy = "always"
max_restarts = 10
backoff_base_ms = 2000
backoff_max_ms = 120000
startup_delay_ms = 2000
core = true
"#
    );

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(config_path, config)?;
    Ok(())
}

fn resolve_source_dir(system_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Priority 1: explicit env override
    if let Ok(explicit) = std::env::var("HARMONIA_SOURCE_DIR") {
        let p = PathBuf::from(explicit);
        if is_runtime_root(&p) {
            return Ok(p);
        }
    }

    let stored_source = harmonia_config_store::get_config("harmonia-cli", "global", "source-dir")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .filter(|path| is_runtime_root(path));
    let installed_share = crate::paths::share_dir()
        .ok()
        .filter(|path| is_runtime_root(path));

    // Installed runtime should default to the installed share tree unless
    // source-rewrite is explicitly enabled or the user set an override.
    if is_installed_binary() && !source_rewrite_enabled() {
        if let Some(share) = installed_share.clone() {
            return Ok(share);
        }
    }

    // Priority 2: Config-store (set during setup)
    if let Some(stored) = stored_source {
        return Ok(stored);
    }

    // Priority 3: Current directory (developer-local workflow)
    let cwd = std::env::current_dir()?;
    if is_runtime_root(&cwd) {
        return Ok(cwd);
    }

    // Priority 4: Platform-standard share dir (~/.local/share/harmonia/)
    if let Some(share) = installed_share {
        return Ok(share);
    }

    // Priority 5: Legacy install location (~/.harmoniis/harmonia) — migration compat
    if is_runtime_root(system_dir) {
        return Ok(system_dir.to_path_buf());
    }

    // Priority 6: Walk up from binary location
    let exe = std::env::current_exe()?;
    let mut dir = exe.parent().unwrap().to_path_buf();

    for _ in 0..10 {
        if is_runtime_root(&dir) {
            return Ok(dir);
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    Err("cannot find Harmonia source directory — run `harmonia setup` first".into())
}
