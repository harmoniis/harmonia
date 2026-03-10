use console::style;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(env: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Validate environment
    match env {
        "test" | "dev" | "prod" => {}
        _ => return Err(format!("invalid environment: {} (use test, dev, or prod)", env).into()),
    }

    // Check system workspace exists
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");
    if !system_dir.join("vault.db").exists() {
        println!(
            "{} Harmonia is not set up yet. Run:",
            style("!").red().bold()
        );
        println!("  {}", style("harmonia setup").cyan().bold());
        return Err("run `harmonia setup` first".into());
    }

    // Bootstrap: set STATE_ROOT so config-store/vault can find their DBs
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());

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
    let wallet_db_path = resolve_wallet_db_path(&home);

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
        "system-dir",
        &system_dir.to_string_lossy(),
    );
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
    println!();

    // Launch SBCL — only pass the minimum bootstrap env vars that the
    // Lisp/Rust layers need before config-store is initialized.
    let status = Command::new("sbcl")
        .arg("--load")
        .arg(&boot_file)
        .arg("--eval")
        .arg("(harmonia:start)")
        // Bootstrap: config-store/vault need these to find their DB files
        .env("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref())
        .env(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db_path.to_string_lossy().as_ref(),
        )
        // Bootstrap: Lisp needs LIB_DIR to locate .dylib/.so files before
        // config-store is loaded (vault.lisp loads first)
        .env("HARMONIA_LIB_DIR", lib_dir.to_string_lossy().as_ref())
        .current_dir(&source_dir)
        .status()?;

    if !status.success() {
        return Err("SBCL exited with error".into());
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

fn resolve_wallet_db_path(home: &Path) -> PathBuf {
    let master = home.join(".harmoniis").join("master.db");
    if master.exists() {
        return master;
    }
    let legacy = home.join(".harmoniis").join("rgb.db");
    if legacy.exists() {
        return legacy;
    }
    master
}

fn is_runtime_root(path: &Path) -> bool {
    path.join("src").join("core").join("boot.lisp").exists()
}

fn shared_lib_ext() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "dylib"
    }
    #[cfg(all(not(target_os = "macos"), target_os = "windows"))]
    {
        "dll"
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        "so"
    }
}

fn required_runtime_libraries() -> Vec<String> {
    let ext = shared_lib_ext();
    [
        "libharmonia_vault",
        "libharmonia_config_store",
        "libharmonia_openrouter",
        "libharmonia_openai",
        "libharmonia_anthropic",
        "libharmonia_xai",
        "libharmonia_google_ai_studio",
        "libharmonia_google_vertex",
        "libharmonia_amazon_bedrock",
        "libharmonia_groq",
        "libharmonia_alibaba",
        "libharmonia_git_ops",
        "libharmonia_harmonic_matrix",
        "libharmonia_admin_intent",
        "libharmonia_gateway",
        "libharmonia_search_exa",
        "libharmonia_search_brave",
        "libharmonia_whisper",
        "libharmonia_elevenlabs",
        "libharmonia_parallel_agents",
        "libharmonia_ouroboros",
        "libharmonia_tui",
    ]
    .iter()
    .map(|name| format!("{name}.{ext}"))
    .collect()
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
    let candidate_target = source_dir.join("target").join("release");
    if candidate_target.exists() {
        return candidate_target;
    }
    let candidate_lib = source_dir.join("lib");
    if candidate_lib.exists() {
        return candidate_lib;
    }
    let parent_lib = source_dir
        .parent()
        .map(|p| p.join("lib"))
        .unwrap_or_else(|| source_dir.join("lib"));
    if parent_lib.exists() {
        return parent_lib;
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

fn resolve_source_dir(system_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Priority 1: Config-store (set during setup)
    if let Ok(Some(stored)) =
        harmonia_config_store::get_config("harmonia-cli", "global", "source-dir")
    {
        let p = PathBuf::from(&stored);
        if is_runtime_root(&p) {
            return Ok(p);
        }
    }

    // Priority 2: Current directory (developer-local workflow)
    let cwd = std::env::current_dir()?;
    if is_runtime_root(&cwd) {
        return Ok(cwd);
    }

    // Priority 3: Standard install location (~/.harmoniis/harmonia)
    if is_runtime_root(system_dir) {
        return Ok(system_dir.to_path_buf());
    }
    let installed_src = system_dir.join("src");
    if is_runtime_root(&installed_src) {
        return Ok(installed_src);
    }

    // Priority 4: Walk up from binary location
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
