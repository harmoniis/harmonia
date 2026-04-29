//! Interactive setup wizard — the main `harmonia setup` flow.

use console::style;
use dialoguer::Input;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::helpers::{
    check_command, copy_dir_recursive, ensure_harmoniis_wallet, find_source_dir,
    install_cdylibs, install_quicklisp, read_existing_workspace, read_masked,
};
use super::gateway_config::generate_gateway_config;
use super::optional;
use super::providers_config::configure_llm_providers;
use super::resolve_configured_modules;
use super::seed_policy::configure_model_seed_policy;
use crate::paths::InstallProfile;

pub fn run_seeds_only() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", style(super::BANNER).cyan().bold());
    println!("  {}", style("Seed model policy setup").dim());
    println!();

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());
    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    println!("  {} Updating model seeds in {}", style("->").cyan().bold(), style(system_dir.join("config.db").display()).green());
    configure_model_seed_policy(&[])?;
    println!();
    println!("  {} Seed setup complete.", style("✓").green().bold());
    println!("  Re-run anytime with {}", style("harmonia setup --seeds").cyan().bold());
    println!();
    Ok(())
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", style(super::BANNER).cyan().bold());
    println!("  {}", style("Distributed evolutionary homoiconic self-improving agent").dim());
    println!();

    // ---- REQUIRED ----
    println!("  {}", style("--- Required ---------------------------------------------------").dim());

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");
    let lib_dir = crate::paths::lib_dir()?;
    let share_dir = crate::paths::share_dir()?;
    let node_identity = crate::paths::current_node_identity()?;
    println!("  {} User data:     {}", style("[1/4]").bold().dim(), style(system_dir.display()).green());
    println!("       Libraries:   {}", style(lib_dir.display()).green());
    println!("       App data:    {}", style(share_dir.display()).green());
    println!("       Node:        {} ({}, {})", style(&node_identity.label).green(), node_identity.role.as_str(), node_identity.install_profile.as_str());
    fs::create_dir_all(&system_dir)?;
    fs::create_dir_all(system_dir.join("config"))?;
    fs::create_dir_all(system_dir.join("frontends"))?;
    fs::create_dir_all(share_dir.join("genesis"))?;
    crate::paths::ensure_node_layout(&node_identity)?;

    // Step 2: SBCL + Quicklisp
    println!("  {} Checking runtime dependencies...", style("[2/4]").bold().dim());
    if !check_command("sbcl") {
        println!("    {} SBCL not found. Install it:", style("!").red().bold());
        println!("      macOS:        brew install sbcl\n      Ubuntu/Debian: sudo apt install sbcl\n      Fedora:       sudo dnf install sbcl\n      FreeBSD:      sudo pkg install sbcl\n      Arch:         sudo pacman -S sbcl");
        println!("\n    Install SBCL and re-run: harmonia setup");
        return Err("SBCL is required".into());
    }
    println!("    {} SBCL found", style("✓").green().bold());
    let quicklisp_path = home.join("quicklisp").join("setup.lisp");
    if !quicklisp_path.exists() {
        println!("    {} Quicklisp not found. Installing...", style("->").yellow().bold());
        install_quicklisp()?;
    }
    println!("    {} Quicklisp found", style("✓").green().bold());

    // Step 3: Wallet + vault + config-store
    println!();
    let wallet_path = ensure_harmoniis_wallet()?;
    let wallet_root = wallet_path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    std::env::set_var("HARMONIA_WALLET_ROOT", wallet_root.to_string_lossy().to_string());
    std::env::set_var("HARMONIA_VAULT_WALLET_DB", wallet_path.to_string_lossy().to_string());
    println!("    {} Wallet identity root: {}", style("✓").green().bold(), wallet_path.display());

    let vault_path = system_dir.join("vault.db");
    std::env::set_var("HARMONIA_VAULT_DB", vault_path.to_string_lossy().as_ref());
    std::env::set_var("HARMONIA_STATE_ROOT", system_dir.to_string_lossy().as_ref());
    harmonia_vault::init_from_env().map_err(|e| format!("vault init failed: {e}"))?;
    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
    };
    for (s, k, v) in [
        ("global", "state-root", system_dir.to_string_lossy().to_string()),
        ("global", "system-dir", system_dir.to_string_lossy().to_string()),
        ("global", "wallet-root", wallet_root.to_string_lossy().to_string()),
        ("global", "wallet-db", wallet_path.to_string_lossy().to_string()),
        ("global", "lib-dir", lib_dir.to_string_lossy().to_string()),
        ("global", "share-dir", share_dir.to_string_lossy().to_string()),
        ("node", "label", node_identity.label.clone()),
        ("node", "hostname", node_identity.hostname.clone()),
        ("node", "role", node_identity.role.as_str().to_string()),
        ("node", "install-profile", node_identity.install_profile.as_str().to_string()),
    ] { cs(s, k, &v)?; }
    cs("node", "sessions-root", &crate::paths::node_sessions_dir(&node_identity.label)?.to_string_lossy())?;
    cs("node", "pairings-root", &crate::paths::node_pairings_dir(&node_identity.label)?.to_string_lossy())?;
    cs("node", "memory-root", &crate::paths::node_memory_dir(&node_identity.label)?.to_string_lossy())?;
    if matches!(node_identity.install_profile, InstallProfile::FullAgent) {
        if let Ok(source_dir) = find_source_dir() {
            cs("global", "source-dir", &source_dir.to_string_lossy())?;
        }
    }

    // Workspace
    let default_workspace = read_existing_workspace(&system_dir).unwrap_or_else(|| home.join("workspace").to_string_lossy().to_string());
    let workspace: String = Input::new().with_prompt(format!("  {} User workspace directory", style("[3/4]").bold().dim())).default(default_workspace).interact_text()?;
    let workspace_path = PathBuf::from(&workspace);
    fs::create_dir_all(&workspace_path)?;
    fs::write(system_dir.join("config").join("workspace.sexp"), format!("(:workspace\n  (:system-dir \"{}\")\n  (:user-workspace \"{}\"))\n", system_dir.display(), workspace_path.display()))?;

    // Step 4: LLM provider
    println!("\n  {} LLM provider credentials (at least one required)", style("[4/4]").bold().dim());
    let configured_providers = configure_llm_providers()?;
    configure_model_seed_policy(&configured_providers)?;

    // ---- OPTIONAL ----
    println!();
    println!("  {}", style("--- Optional (Enter to skip, configure later with `harmonia setup`) --").dim());
    let enabled_frontends: Vec<&str> = vec!["tui"];
    println!();
    println!("  {} Frontends are configured from the interactive CLI only.", style("->").cyan().bold());
    println!("    {}", style("Finish setup, start Harmonia, then use /menu -> Frontends").dim());

    println!("\n  Optional tool API keys (Enter to skip):");
    for (symbol, prompt) in [("exa-api-key", "Exa search API key"), ("brave-api-key", "Brave search API key"), ("elevenlabs-api-key", "ElevenLabs API key")] {
        let existing = harmonia_vault::has_secret_for_symbol(symbol);
        let label = if existing { format!("    {} [configured] (Enter to keep)", prompt) } else { format!("    {}", prompt) };
        let value = read_masked(&label)?;
        if !value.is_empty() { harmonia_vault::set_secret_for_symbol(symbol, &value).map_err(|e| format!("vault write failed for {}: {e}", symbol))?; }
    }
    optional::configure_langsmith_observability()?;
    optional::configure_evolution_profile(&home)?;

    let enabled_modules = resolve_configured_modules();
    if !enabled_modules.is_empty() {
        let csv = enabled_modules.join(",");
        harmonia_config_store::set_config("harmonia-cli", "runtime", "components", &csv).map_err(|e| format!("failed to persist runtime components: {e}"))?;
        println!("    {} Runtime modules auto-enabled: {}", style("✓").green().bold(), style(&csv).dim());
    }

    // ---- Finalize ----
    finalize(
        &system_dir,
        &share_dir,
        &lib_dir,
        &enabled_frontends,
        &cs,
        &home,
        &workspace_path,
        node_identity.install_profile,
    )
}

fn finalize(
    system_dir: &Path,
    share_dir: &Path,
    lib_dir: &Path,
    enabled_frontends: &[&str],
    cs: &dyn Fn(&str, &str, &str) -> Result<(), Box<dyn std::error::Error>>,
    home: &Path,
    workspace_path: &Path,
    install_profile: InstallProfile,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n  {} Finalizing...", style("->").cyan().bold());
    let gateway_config = generate_gateway_config(enabled_frontends);
    fs::write(system_dir.join("config").join("baseband.sexp"), &gateway_config)?;
    fs::write(system_dir.join("config").join("gateway-frontends.sexp"), &gateway_config)?;

    if matches!(install_profile, InstallProfile::FullAgent) {
        let hint = concat!(
            "For a full local agent install, you need the Harmonia Lisp/runtime tree:\n",
            "    • Run this repository's ",
            "`scripts/install.sh`, or\n",
            "    • Clone harmonia and run `harmonia setup` from the repo root, or\n",
            "    • Set ",
            "`HARMONIA_SOURCE_DIR` ",
            "to that repository root (must contain ",
            "`src/core/boot.lisp`",
            ").",
        );

        let source_dir = find_source_dir().map_err(|e| format!("{e}\n\n{hint}"))?;
        let genesis_src = source_dir.join("doc").join("genesis");
        let genesis_dst = share_dir.join("genesis");
        if genesis_src.exists() && genesis_src != genesis_dst {
            copy_dir_recursive(&genesis_src, &genesis_dst)?;
            println!("    {} Evolution knowledge installed", style("✓").green().bold());
        }

        let lisp_src = source_dir.join("src");
        if lisp_src.exists() {
            let share_src = crate::paths::source_dir()?;
            if lisp_src != share_src {
                copy_dir_recursive(&lisp_src, &share_src)?;
            }
            let config_src = source_dir.join("config");
            let config_dst = share_dir.join("config");
            if config_src.exists() && config_src != config_dst {
                copy_dir_recursive(&config_src, &config_dst)?;
            }
            cs("global", "source-dir", &share_dir.to_string_lossy())?;
            println!("    {} Lisp source installed to {}", style("✓").green().bold(), share_dir.display());
        }

        if !crate::paths::is_runtime_root(share_dir) {
            return Err(format!(
                "Lisp runtime is not installed under {} (missing {}). Re-run setup from the repo or release installer.\n\n{hint}",
                share_dir.display(),
                share_dir.join("src/core/boot.lisp").display()
            ).into());
        }

        if source_dir.join("Cargo.toml").exists() {
            println!("    {} Building runtime artifacts...", style("->").cyan().bold());
            let build_status =
                Command::new("cargo").args(["build", "--workspace", "--release"]).current_dir(&source_dir).status()?;
            if !build_status.success() {
                return Err("failed to build runtime artifacts".into());
            }
            println!("    {} Runtime artifacts built", style("✓").green().bold());
            let target_release = source_dir.join("target").join("release");
            install_cdylibs(&target_release, lib_dir)?;
            println!(
                "    {} Libraries installed to {}",
                style("✓").green().bold(),
                lib_dir.display()
            );
            let bin_name = if cfg!(target_os = "windows") { "harmonia.exe" } else { "harmonia" };
            let built_bin = target_release.join(bin_name);
            if built_bin.exists() {
                let dest_bin = home.join(".local").join("bin").join(bin_name);
                fs::create_dir_all(dest_bin.parent().unwrap())?;
                fs::copy(&built_bin, &dest_bin)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dest_bin, fs::Permissions::from_mode(0o755));
                }
                println!("    {} Binary installed to {}", style("✓").green().bold(), dest_bin.display());
            }
        }
    }

    println!();
    println!("  {} Setup complete!", style("✓").green().bold());
    println!();
    println!("  User data:        {}", style(system_dir.display()).green());
    println!("  Libraries:        {}", style(lib_dir.display()).green());
    println!("  App data:         {}", style(share_dir.display()).green());
    println!("  User workspace:   {}", style(workspace_path.display()).green());
    println!("  Vault:            {}", style(system_dir.join("vault.db").display()).green());
    println!("  Config DB:        {}", style(system_dir.join("config.db").display()).green());
    println!();
    println!("  Start the agent:");
    println!("    {}", style("harmonia start").cyan().bold());
    println!();
    println!("  {}", style("  To add frontends/tools later: harmonia setup").dim());
    println!();
    Ok(())
}
