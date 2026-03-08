use console::style;
use dialoguer::Confirm;
use std::fs;
use std::path::{Path, PathBuf};

/// Shell rc files to check for Harmonia PATH/env blocks.
const SHELL_RC_FILES: &[&str] = &[".zshrc", ".bashrc"];

/// Marker comments that delimit the Harmonia block in shell rc files.
const BLOCK_START: &str = "# Harmonia agent";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let harmoniis_dir = home.join(".harmoniis");
    let harmonia_dir = harmoniis_dir.join("harmonia");
    let symlink_path = home.join(".local").join("bin").join("harmonia");
    let vault_db = harmonia_dir.join("vault.db");
    let vault_backup = harmoniis_dir.join("harmonia-vault-backup.db");

    println!();
    println!(
        "  {}",
        style("Harmonia Uninstall").bold().red()
    );
    println!();

    // ── Describe what will be removed ────────────────────────────────
    println!("  The following will be {} :", style("removed").red().bold());
    println!();

    let mut items_to_remove: Vec<String> = Vec::new();

    if harmonia_dir.exists() {
        println!(
            "    {} {}",
            style("•").red(),
            style(harmonia_dir.display()).dim()
        );
        items_to_remove.push(format!("{}", harmonia_dir.display()));
    }

    if symlink_path.exists() || symlink_path.symlink_metadata().is_ok() {
        println!(
            "    {} {} (symlink)",
            style("•").red(),
            style(symlink_path.display()).dim()
        );
        items_to_remove.push(format!("{}", symlink_path.display()));
    }

    let rc_files_with_block = find_rc_files_with_block(&home);
    for rc in &rc_files_with_block {
        println!(
            "    {} Harmonia block in {}",
            style("•").red(),
            style(rc.display()).dim()
        );
        items_to_remove.push(format!("Harmonia block in {}", rc.display()));
    }

    if items_to_remove.is_empty() {
        println!(
            "  {} Nothing to uninstall — Harmonia does not appear to be installed.",
            style("✓").green().bold()
        );
        return Ok(());
    }

    // ── Describe what will be preserved ──────────────────────────────
    println!();
    println!(
        "  The following will be {} :",
        style("preserved").green().bold()
    );
    println!();

    let wallet_dir = harmoniis_dir.join("wallet");
    let master_db = harmoniis_dir.join("master.db");

    if wallet_dir.exists() {
        println!(
            "    {} {}",
            style("•").green(),
            style(wallet_dir.display()).dim()
        );
    }
    if master_db.exists() {
        println!(
            "    {} {}",
            style("•").green(),
            style(master_db.display()).dim()
        );
    }
    if vault_db.exists() {
        println!(
            "    {} {} (backed up to {})",
            style("•").green(),
            style("vault.db").dim(),
            style(vault_backup.display()).dim()
        );
    }

    // Show any other harmoniis files/dirs that exist (besides harmonia/)
    if let Ok(entries) = fs::read_dir(&harmoniis_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            // Skip harmonia dir (being removed) and items already listed
            if name_str == "harmonia"
                || name_str == "wallet"
                || name_str == "master.db"
                || name_str == "harmonia-vault-backup.db"
            {
                continue;
            }
            println!(
                "    {} {}",
                style("•").green(),
                style(entry.path().display()).dim()
            );
        }
    }

    println!();

    // ── Confirmation ─────────────────────────────────────────────────
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "  {} Proceed with uninstall?",
            style("?").yellow().bold()
        ))
        .default(false)
        .interact()?;

    if !confirmed {
        println!(
            "  {} Uninstall cancelled.",
            style("✗").red().bold()
        );
        return Ok(());
    }

    println!();

    // ── Step 1: Back up vault.db ─────────────────────────────────────
    if vault_db.exists() {
        fs::copy(&vault_db, &vault_backup)?;
        println!(
            "  {} Backed up vault.db to {}",
            style("✓").green().bold(),
            style(vault_backup.display()).cyan()
        );
    }

    // ── Step 2: Remove ~/.harmoniis/harmonia/ ────────────────────────
    if harmonia_dir.exists() {
        fs::remove_dir_all(&harmonia_dir)?;
        println!(
            "  {} Removed {}",
            style("✓").green().bold(),
            harmonia_dir.display()
        );
    }

    // ── Step 3: Remove symlink ───────────────────────────────────────
    if symlink_path.symlink_metadata().is_ok() {
        fs::remove_file(&symlink_path)?;
        println!(
            "  {} Removed symlink {}",
            style("✓").green().bold(),
            symlink_path.display()
        );
    }

    // ── Step 4: Clean shell rc files ─────────────────────────────────
    for rc in &rc_files_with_block {
        remove_harmonia_block(rc)?;
        println!(
            "  {} Cleaned Harmonia block from {}",
            style("✓").green().bold(),
            rc.display()
        );
    }

    // ── Done ─────────────────────────────────────────────────────────
    println!();
    println!(
        "  {} Harmonia has been uninstalled.",
        style("✓").green().bold()
    );
    if vault_backup.exists() {
        println!(
            "  Vault backup: {}",
            style(vault_backup.display()).cyan()
        );
    }
    println!(
        "  Wallet data in {} was {}.",
        style("~/.harmoniis/").cyan(),
        style("not touched").green().bold()
    );
    println!();

    Ok(())
}

/// Find shell rc files that contain the Harmonia block.
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

/// Remove the "# Harmonia agent" block from a shell rc file.
///
/// The block is expected to start with a line containing `# Harmonia agent`
/// and includes contiguous non-empty lines that set HARMONIA_HOME or modify
/// PATH for harmonia, plus any immediately following blank line.
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
            // Inside the block: skip lines that are part of the Harmonia env
            // setup (export HARMONIA_HOME, PATH additions, blank lines between
            // them, or additional # Harmonia comments).
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("export HARMONIA_HOME")
                || trimmed.starts_with("HARMONIA_HOME=")
                || (trimmed.contains("HARMONIA_HOME") && trimmed.contains("PATH"))
                || (trimmed.contains(".harmoniis/harmonia") && trimmed.contains("PATH"))
                || (trimmed.contains(".local/bin") && trimmed.contains("PATH") && trimmed.contains("harmonia"))
                || trimmed.starts_with("# Harmonia")
            {
                continue;
            }
            // Line is not part of the block — stop skipping
            in_block = false;
            output_lines.push(line);
        }
    }

    // Remove trailing blank lines that may have been left
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
