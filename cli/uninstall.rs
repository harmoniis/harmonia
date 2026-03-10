use console::style;
use dialoguer::Confirm;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Shell rc files to check for Harmonia PATH/env blocks.
const SHELL_RC_FILES: &[&str] = &[".zshrc", ".bashrc"];

/// Marker comments that delimit the Harmonia block in shell rc files.
const BLOCK_START: &str = "# Harmonia agent";

/// Subcommands for `harmonia uninstall`
#[derive(clap::Subcommand, Clone)]
pub enum UninstallAction {
    /// Export evolution state (versions, snapshots, config) to a portable archive
    #[command(name = "evolution-export")]
    EvolutionExport {
        /// Output path for the evolution archive (.tar.gz)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import a previously exported evolution archive into a fresh installation
    #[command(name = "evolution-import")]
    EvolutionImport {
        /// Path to the evolution archive (.tar.gz) to import
        path: String,
        /// Merge with existing evolution state instead of replacing
        #[arg(long)]
        merge: bool,
    },
}

pub fn run(action: Option<UninstallAction>) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        Some(UninstallAction::EvolutionExport { output }) => run_evolution_export(output),
        Some(UninstallAction::EvolutionImport { path, merge }) => {
            run_evolution_import(&path, merge)
        }
        None => run_uninstall(),
    }
}

// ─── Evolution Export ─────────────────────────────────────────────────────

fn run_evolution_export(output: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let data_dir = crate::paths::data_dir()?;
    let share_dir = crate::paths::share_dir()?;

    // Gather evolution directories
    let evolution_dir = data_dir.join("evolution");
    let share_evolution = share_dir.join("genesis");

    if !evolution_dir.exists() && !share_evolution.exists() {
        println!(
            "  {} No evolution state found — nothing to export.",
            style("!").yellow().bold()
        );
        return Ok(());
    }

    // Default output path
    let timestamp = chrono_timestamp();
    let default_name = format!("harmonia-evolution-{}.tar.gz", timestamp);
    let output_path = PathBuf::from(
        output.unwrap_or_else(|| home.join(&default_name).to_string_lossy().to_string()),
    );

    println!(
        "  {} Exporting evolution state...",
        style("→").cyan().bold()
    );

    // Build a temp staging directory
    let staging = std::env::temp_dir().join(format!("harmonia-evolution-export-{}", timestamp));
    fs::create_dir_all(&staging)?;

    // Copy evolution versions + latest + version.sexp
    if evolution_dir.exists() {
        copy_dir_recursive(&evolution_dir, &staging.join("evolution"))?;
    }

    // Copy genesis knowledge
    if share_evolution.exists() {
        copy_dir_recursive(&share_evolution, &staging.join("genesis"))?;
    }

    // Export config-store evolution keys
    export_config_keys(&staging)?;

    // Export evolution metadata
    let meta = format!(
        "(:export-version 1\n :timestamp \"{}\"\n :platform \"{}\")\n",
        timestamp,
        std::env::consts::OS,
    );
    fs::write(staging.join("manifest.sexp"), &meta)?;

    // Create tar.gz
    let status = Command::new("tar")
        .args([
            "-czf",
            &output_path.to_string_lossy(),
            "-C",
            &staging.parent().unwrap().to_string_lossy(),
            &staging.file_name().unwrap().to_string_lossy().to_string(),
        ])
        .status()?;

    // Cleanup staging
    let _ = fs::remove_dir_all(&staging);

    if !status.success() {
        return Err("failed to create evolution archive".into());
    }

    println!(
        "  {} Evolution exported to: {}",
        style("✓").green().bold(),
        style(output_path.display()).cyan()
    );
    println!();
    println!("  To import into a fresh install:");
    println!(
        "    {}",
        style(format!(
            "harmonia uninstall evolution-import {}",
            output_path.display()
        ))
        .cyan()
        .bold()
    );
    println!(
        "    {}",
        style(format!(
            "harmonia uninstall evolution-import {} --merge",
            output_path.display()
        ))
        .dim()
    );

    Ok(())
}

// ─── Evolution Import ─────────────────────────────────────────────────────

fn run_evolution_import(path: &str, merge: bool) -> Result<(), Box<dyn std::error::Error>> {
    let archive = PathBuf::from(path);
    if !archive.exists() {
        return Err(format!("archive not found: {}", path).into());
    }

    let data_dir = crate::paths::data_dir()?;
    let share_dir = crate::paths::share_dir()?;

    // Extract to temp dir
    let staging = std::env::temp_dir().join("harmonia-evolution-import");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging)?;

    let status = Command::new("tar")
        .args(["-xzf", path, "-C", &staging.to_string_lossy()])
        .status()?;
    if !status.success() {
        let _ = fs::remove_dir_all(&staging);
        return Err("failed to extract evolution archive".into());
    }

    // Find the extracted root (tar creates a subdirectory)
    let extracted_root = find_extracted_root(&staging)?;

    // Verify manifest
    let manifest = extracted_root.join("manifest.sexp");
    if !manifest.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Err("invalid evolution archive — missing manifest.sexp".into());
    }

    println!(
        "  {} Importing evolution state{}...",
        style("→").cyan().bold(),
        if merge { " (merge mode)" } else { "" }
    );

    // Import evolution versions
    let src_evolution = extracted_root.join("evolution");
    if src_evolution.exists() {
        let dst_evolution = data_dir.join("evolution");
        if merge && dst_evolution.exists() {
            merge_evolution_dirs(&src_evolution, &dst_evolution)?;
        } else {
            if dst_evolution.exists() {
                fs::remove_dir_all(&dst_evolution)?;
            }
            copy_dir_recursive(&src_evolution, &dst_evolution)?;
        }
    }

    // Import genesis
    let src_genesis = extracted_root.join("genesis");
    if src_genesis.exists() {
        let dst_genesis = share_dir.join("genesis");
        if !merge && dst_genesis.exists() {
            fs::remove_dir_all(&dst_genesis)?;
        }
        copy_dir_recursive(&src_genesis, &dst_genesis)?;
    }

    // Import config keys
    let config_export = extracted_root.join("config-evolution.sexp");
    if config_export.exists() {
        import_config_keys(&config_export)?;
    }

    let _ = fs::remove_dir_all(&staging);

    println!(
        "  {} Evolution state imported successfully.",
        style("✓").green().bold()
    );
    if merge {
        println!(
            "  {} Merged with existing evolution — version numbers may need reconciliation on next boot.",
            style("i").cyan().bold()
        );
    }

    Ok(())
}

// ─── Uninstall ────────────────────────────────────────────────────────────

fn run_uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let data_dir = crate::paths::data_dir()?;

    println!();
    println!("  {}", style("Harmonia Uninstall").bold().red());
    println!();

    // ── Check evolution safety gates ─────────────────────────────────

    let has_evolution =
        data_dir.join("evolution").exists() && data_dir.join("evolution").join("versions").exists();
    let evolution_version = read_evolution_version(&data_dir);

    if has_evolution && evolution_version > 0 {
        println!(
            "  {} Evolution state detected (v{}).",
            style("!").yellow().bold(),
            evolution_version
        );

        // Initialize config-store to check distributed settings
        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
        let _ = harmonia_config_store::init_v2();

        let source_pushed = check_source_pushed(&data_dir);
        let distributed_propagated = check_distributed_propagated();

        if !source_pushed && !distributed_propagated {
            // Local-only evolution — warn strongly
            println!();
            println!(
                "  {} This agent has locally evolved code that has {}.",
                style("WARNING").red().bold(),
                style("NOT been pushed to git or distributed store").red()
            );
            println!(
                "  Uninstalling will {} lose this evolution permanently.",
                style("irreversibly").red().bold()
            );
            println!();
            println!("  You can back up your evolution first:");
            println!(
                "    {}",
                style("harmonia uninstall evolution-export").cyan().bold()
            );
            println!("  Then reimport after reinstalling:");
            println!(
                "    {}",
                style("harmonia uninstall evolution-import <archive.tar.gz>")
                    .cyan()
                    .bold()
            );
            println!();

            let backup_now = Confirm::new()
                .with_prompt(format!(
                    "  {} Export evolution backup before uninstalling?",
                    style("?").yellow().bold()
                ))
                .default(true)
                .interact()?;

            if backup_now {
                run_evolution_export(None)?;
                println!();
            }

            let confirmed = Confirm::new()
                .with_prompt(format!(
                    "  {} Are you 100% sure you want to uninstall and LOSE local evolution?",
                    style("?").red().bold()
                ))
                .default(false)
                .interact()?;

            if !confirmed {
                println!("  {} Uninstall cancelled.", style("✗").red().bold());
                return Ok(());
            }
        } else {
            // Evolution is safely backed up somewhere
            if source_pushed {
                println!(
                    "    {} Source evolution committed and pushed to git.",
                    style("✓").green().bold()
                );
            }
            if distributed_propagated {
                println!(
                    "    {} Binary evolution propagated to distributed store.",
                    style("✓").green().bold()
                );
            }
            println!();
        }
    }

    // ── Collect items to remove ──────────────────────────────────────

    let lib_dir = crate::paths::lib_dir().ok();
    let share_dir = crate::paths::share_dir().ok();
    let log_dir = crate::paths::log_dir().ok();
    let run_dir = crate::paths::run_dir().ok();
    let bin_path = home.join(".local").join("bin").join("harmonia");

    println!(
        "  The following will be {} :",
        style("removed").red().bold()
    );
    println!();

    let mut removals: Vec<(PathBuf, &str)> = Vec::new();

    if let Some(ref d) = lib_dir {
        if d.exists() {
            removals.push((d.clone(), "libraries"));
        }
    }
    if let Some(ref d) = share_dir {
        if d.exists() {
            removals.push((d.clone(), "app data (source, docs)"));
        }
    }
    if let Some(ref d) = log_dir {
        if d.exists() {
            removals.push((d.clone(), "logs"));
        }
    }
    if let Some(ref d) = run_dir {
        if d.exists() {
            removals.push((d.clone(), "runtime (PID, socket)"));
        }
    }
    if bin_path.exists() || bin_path.symlink_metadata().is_ok() {
        removals.push((bin_path.clone(), "binary"));
    }

    // Evolution data from user data dir
    let evolution_dir = data_dir.join("evolution");
    if evolution_dir.exists() {
        removals.push((evolution_dir, "evolution state"));
    }

    let rc_files = find_rc_files_with_block(&home);
    for rc in &rc_files {
        println!(
            "    {} Harmonia block in {} (shell config)",
            style("•").red(),
            style(rc.display()).dim()
        );
    }

    for (path, label) in &removals {
        println!(
            "    {} {} ({})",
            style("•").red(),
            style(path.display()).dim(),
            label
        );
    }

    if removals.is_empty() && rc_files.is_empty() {
        println!(
            "  {} Nothing to uninstall — Harmonia does not appear to be installed.",
            style("✓").green().bold()
        );
        return Ok(());
    }

    // ── Describe what will NOT be touched ──────────────────────────
    println!();
    println!(
        "  {} will {} be touched:",
        style("~/.harmoniis/").cyan(),
        style("NOT").green().bold()
    );
    println!("    vault.db, config.db, metrics.db, config/, frontends/, state/");
    println!("    (Run `harmonia setup` to reconfigure after reinstalling)");
    println!();

    // ── Confirmation ──────────────────────────────────────────────
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "  {} Proceed with uninstall?",
            style("?").yellow().bold()
        ))
        .default(false)
        .interact()?;

    if !confirmed {
        println!("  {} Uninstall cancelled.", style("✗").red().bold());
        return Ok(());
    }

    println!();

    // ── Stop daemon if running ────────────────────────────────────
    if let Ok(pid_path) = crate::paths::pid_path() {
        if pid_path.exists() {
            println!("  {} Stopping daemon...", style("→").cyan().bold());
            let _ = crate::stop::run();
        }
    }

    // ── Remove system service if installed ────────────────────────
    let _ = crate::service::uninstall();

    // ── Remove directories ───────────────────────────────────────
    for (path, label) in &removals {
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else if path.exists() || path.symlink_metadata().is_ok() {
            fs::remove_file(path)?;
        }
        println!(
            "  {} Removed {} ({})",
            style("✓").green().bold(),
            path.display(),
            label
        );
    }

    // ── Clean shell rc files ──────────────────────────────────────
    for rc in &rc_files {
        remove_harmonia_block(rc)?;
        println!(
            "  {} Cleaned Harmonia block from {}",
            style("✓").green().bold(),
            rc.display()
        );
    }

    // ── Also remove legacy dirs from ~/.harmoniis/harmonia/ ───────
    for legacy_dir in &["bin", "lib", "src", "doc"] {
        let legacy = data_dir.join(legacy_dir);
        if legacy.exists() {
            fs::remove_dir_all(&legacy)?;
            println!(
                "  {} Removed legacy {} (migration cleanup)",
                style("✓").green().bold(),
                legacy.display()
            );
        }
    }
    // Remove stale log from legacy location
    let legacy_log = data_dir.join("harmonia.log");
    if legacy_log.exists() {
        fs::remove_file(&legacy_log)?;
    }

    // ── Done ──────────────────────────────────────────────────────
    println!();
    println!(
        "  {} Harmonia has been uninstalled.",
        style("✓").green().bold()
    );
    println!(
        "  User data in {} was {}.",
        style("~/.harmoniis/").cyan(),
        style("preserved").green().bold()
    );
    println!();
    println!("  To reinstall:");
    println!(
        "    {}",
        style("cargo install harmonia && harmonia setup")
            .cyan()
            .bold()
    );
    println!();

    Ok(())
}

// ─── Evolution safety checks ──────────────────────────────────────────────

fn read_evolution_version(data_dir: &Path) -> u32 {
    let version_file = data_dir.join("evolution").join("version.sexp");
    if let Ok(content) = fs::read_to_string(&version_file) {
        content.trim().parse::<u32>().unwrap_or(0)
    } else {
        0
    }
}

fn check_source_pushed(data_dir: &Path) -> bool {
    // Check if the source-rewrite git repo has been pushed
    // Look for config-store key or check git status
    if let Ok(Some(source_dir)) =
        harmonia_config_store::get_config("harmonia-cli", "global", "source-dir")
    {
        let source_path = PathBuf::from(&source_dir);
        if source_path.join(".git").exists() {
            // Check if HEAD is pushed to a remote
            let output = Command::new("git")
                .args(["log", "--oneline", "@{u}..HEAD"])
                .current_dir(&source_path)
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let unpushed = String::from_utf8_lossy(&out.stdout);
                    return unpushed.trim().is_empty(); // empty = all pushed
                }
            }
        }
    }

    // Also check if evolution docs are in a git repo
    let evolution_dir = data_dir.join("evolution");
    if evolution_dir.join(".git").exists() {
        let output = Command::new("git")
            .args(["log", "--oneline", "@{u}..HEAD"])
            .current_dir(&evolution_dir)
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().is_empty();
            }
        }
    }

    false
}

fn check_distributed_propagated() -> bool {
    // Check config-store for distributed evolution settings
    if let Ok(Some(enabled)) = harmonia_config_store::get_config(
        "harmonia-cli",
        "evolution",
        "distributed-evolution-enabled",
    ) {
        let is_enabled = matches!(enabled.to_lowercase().as_str(), "1" | "true" | "yes" | "on");
        if !is_enabled {
            return false;
        }
        // If distributed is enabled AND bucket is configured, assume propagated
        if let Ok(Some(bucket)) = harmonia_config_store::get_config(
            "harmonia-cli",
            "evolution",
            "distributed-store-bucket",
        ) {
            return !bucket.is_empty();
        }
    }
    false
}

// ─── Config key export/import ─────────────────────────────────────────────

fn export_config_keys(staging: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Export evolution-related config keys as s-expression
    let keys = [
        ("evolution", "mode"),
        ("evolution", "source-rewrite-enabled"),
        ("evolution", "distributed-evolution-enabled"),
        ("evolution", "distributed-store-kind"),
        ("evolution", "distributed-store-bucket"),
        ("evolution", "distributed-store-prefix"),
    ];
    let mut entries = Vec::new();
    for (scope, key) in &keys {
        if let Ok(Some(value)) = harmonia_config_store::get_config("harmonia-cli", scope, key) {
            entries.push(format!(
                "  (:scope \"{}\" :key \"{}\" :value \"{}\")",
                scope, key, value
            ));
        }
    }
    if !entries.is_empty() {
        let content = format!("(:config-keys\n{})\n", entries.join("\n"));
        fs::write(staging.join("config-evolution.sexp"), content)?;
    }
    Ok(())
}

fn import_config_keys(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Best-effort: re-set evolution config keys from export
    let content = fs::read_to_string(path)?;
    // Simple parser: find :scope "X" :key "Y" :value "Z" patterns
    for line in content.lines() {
        let line = line.trim();
        if !line.starts_with("(:scope") {
            continue;
        }
        if let (Some(scope), Some(key), Some(value)) = (
            extract_sexp_string(line, ":scope"),
            extract_sexp_string(line, ":key"),
            extract_sexp_string(line, ":value"),
        ) {
            let _ = harmonia_config_store::set_config("harmonia-cli", &scope, &key, &value);
        }
    }
    Ok(())
}

fn extract_sexp_string(line: &str, key: &str) -> Option<String> {
    let needle = format!("{} \"", key);
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

// ─── Helpers ──────────────────────────────────────────────────────────────

fn chrono_timestamp() -> String {
    // Use system time for timestamp without chrono dep
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

fn find_extracted_root(staging: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // tar creates a subdirectory — find it
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        if entry.path().is_dir() {
            return Ok(entry.path());
        }
    }
    // Flat extraction
    Ok(staging.to_path_buf())
}

fn merge_evolution_dirs(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Merge versions: copy vN dirs that don't exist in dst
    let src_versions = src.join("versions");
    let dst_versions = dst.join("versions");
    if src_versions.exists() {
        fs::create_dir_all(&dst_versions)?;
        for entry in fs::read_dir(&src_versions)? {
            let entry = entry?;
            let name = entry.file_name();
            let dst_dir = dst_versions.join(&name);
            if !dst_dir.exists() {
                copy_dir_recursive(&entry.path(), &dst_dir)?;
            }
        }
    }

    // Merge latest: overwrite with imported if newer version
    let src_latest = src.join("latest");
    let dst_latest = dst.join("latest");
    if src_latest.exists() {
        fs::create_dir_all(&dst_latest)?;
        for entry in fs::read_dir(&src_latest)? {
            let entry = entry?;
            let dst_file = dst_latest.join(entry.file_name());
            fs::copy(entry.path(), &dst_file)?;
        }
    }

    // Take the higher version number
    let src_ver = src.join("version.sexp");
    let dst_ver = dst.join("version.sexp");
    if src_ver.exists() {
        let src_v: u32 = fs::read_to_string(&src_ver)?.trim().parse().unwrap_or(0);
        let dst_v: u32 = fs::read_to_string(&dst_ver)
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(0);
        if src_v > dst_v {
            fs::copy(&src_ver, &dst_ver)?;
        }
    }

    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &dest)?;
        } else {
            fs::copy(&path, &dest)?;
        }
    }
    Ok(())
}

fn find_rc_files_with_block(home: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for name in SHELL_RC_FILES {
        let path = home.join(name);
        if let Ok(content) = fs::read_to_string(&path) {
            if content.contains(BLOCK_START) {
                found.push(path);
            }
        }
    }
    found
}

fn remove_harmonia_block(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let mut output_lines: Vec<&str> = Vec::new();
    let mut in_block = false;

    for line in content.lines() {
        if !in_block {
            if line.contains(BLOCK_START) {
                in_block = true;
                continue;
            }
            output_lines.push(line);
        } else {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("export HARMONIA_HOME")
                || trimmed.starts_with("HARMONIA_HOME=")
                || (trimmed.contains("HARMONIA_HOME") && trimmed.contains("PATH"))
                || (trimmed.contains(".harmoniis/harmonia") && trimmed.contains("PATH"))
                || (trimmed.contains(".local/bin")
                    && trimmed.contains("PATH")
                    && trimmed.contains("harmonia"))
                || trimmed.starts_with("# Harmonia")
            {
                continue;
            }
            in_block = false;
            output_lines.push(line);
        }
    }

    while output_lines.last().map_or(false, |l| l.trim().is_empty()) {
        output_lines.pop();
    }

    let mut result = output_lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    fs::write(path, result)?;
    Ok(())
}
