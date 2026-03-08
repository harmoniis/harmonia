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

    // Runtime env can override source/lib roots for binary+source-rewrite installs.
    let runtime_env_path = system_dir.join("config").join("runtime.env");
    let runtime_env = load_runtime_env_file(&runtime_env_path);
    apply_runtime_dir_overrides(&runtime_env);

    // Find source directory
    let source_dir = find_source_dir()?;
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

    // Set environment variables
    let vault_path = system_dir.join("vault.db");
    let wallet_db_path = resolve_wallet_db_path(&home);

    println!(
        "{} Starting Harmonia (env={})",
        style("→").cyan().bold(),
        style(env).green()
    );
    println!("  source:    {}", source_dir.display());
    println!("  libraries: {}", lib_dir.display());
    println!("  vault:     {}", vault_path.display());
    println!("  wallet:    {}", wallet_db_path.display());
    if !runtime_env.is_empty() {
        println!(
            "  runtime:   {} ({} vars)",
            runtime_env_path.display(),
            runtime_env.len()
        );
    }
    println!("  workspace: {}", system_dir.display());
    println!();

    // Launch SBCL with the boot script
    let mut cmd = Command::new("sbcl");
    cmd.arg("--load")
        .arg(&boot_file)
        .arg("--eval")
        .arg("(harmonia:start)");
    for (k, v) in runtime_env.iter() {
        cmd.env(k, v);
    }
    let status = cmd
        .env("HARMONIA_ENV", env)
        .env("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref())
        .env(
            "HARMONIA_VAULT_WALLET_DB",
            wallet_db_path.to_string_lossy().as_ref(),
        )
        .env("HARMONIA_VAULT_PATH", vault_path.to_string_lossy().as_ref())
        .env("HARMONIA_SYSTEM_DIR", system_dir.to_string_lossy().as_ref())
        .env("HARMONIA_SOURCE_DIR", source_dir.to_string_lossy().as_ref())
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

fn apply_runtime_dir_overrides(runtime_env: &[(String, String)]) {
    for (k, v) in runtime_env {
        if (k == "HARMONIA_SOURCE_DIR" || k == "HARMONIA_LIB_DIR")
            && std::env::var(k).is_err()
            && !v.trim().is_empty()
        {
            std::env::set_var(k, v);
        }
    }
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
    if let Ok(raw) = std::env::var("HARMONIA_LIB_DIR") {
        let p = PathBuf::from(raw);
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

fn load_runtime_env_file(path: &Path) -> Vec<(String, String)> {
    let body = match std::fs::read_to_string(path) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            let key = k.trim();
            let value = v.trim();
            if !key.is_empty() {
                out.push((key.to_string(), value.to_string()));
            }
        }
    }
    out
}

fn find_source_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Priority 1: HARMONIA_SOURCE_DIR env var
    if let Ok(env_dir) = std::env::var("HARMONIA_SOURCE_DIR") {
        let p = PathBuf::from(&env_dir);
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
    if let Some(home) = dirs::home_dir() {
        let installed_root = home.join(".harmoniis").join("harmonia");
        if is_runtime_root(&installed_root) {
            return Ok(installed_root);
        }
        let installed_src = installed_root.join("src");
        if is_runtime_root(&installed_src) {
            return Ok(installed_src);
        }
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

    Err("cannot find Harmonia source directory — set HARMONIA_SOURCE_DIR or run from the project root".into())
}
