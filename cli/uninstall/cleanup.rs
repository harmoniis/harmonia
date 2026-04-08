//! Main uninstall flow: collect removals, confirm, execute.

use console::style;
use dialoguer::Confirm;
use std::fs;
use std::path::PathBuf;

use super::evolution;
use super::helpers::{find_rc_files_with_block, remove_harmonia_block};

pub fn run_uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let data_dir = crate::paths::data_dir()?;

    println!();
    println!("  {}", style("Harmonia Uninstall").bold().red());
    println!();

    // -- Evolution safety gates --
    let has_evolution =
        data_dir.join("evolution").exists() && data_dir.join("evolution").join("versions").exists();
    let evolution_version = evolution::read_evolution_version(&data_dir);

    if has_evolution && evolution_version > 0 {
        println!(
            "  {} Evolution state detected (v{}).",
            style("!").yellow().bold(),
            evolution_version
        );

        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
        let _ = harmonia_config_store::init_v2();

        let source_pushed = evolution::check_source_pushed(&data_dir);
        let distributed_propagated = evolution::check_distributed_propagated();

        if !source_pushed && !distributed_propagated {
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
                evolution::run_evolution_export(None)?;
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
                println!("  {} Uninstall cancelled.", style("x").red().bold());
                return Ok(());
            }
        } else {
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

    // -- Collect items to remove --
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
    if let Some(ref d) = lib_dir { if d.exists() { removals.push((d.clone(), "libraries")); } }
    if let Some(ref d) = share_dir { if d.exists() { removals.push((d.clone(), "app data (source, docs)")); } }
    if let Some(ref d) = log_dir { if d.exists() { removals.push((d.clone(), "logs")); } }
    if let Some(ref d) = run_dir { if d.exists() { removals.push((d.clone(), "runtime (PID, socket)")); } }
    if bin_path.exists() || bin_path.symlink_metadata().is_ok() {
        removals.push((bin_path.clone(), "binary"));
    }

    let evolution_dir = data_dir.join("evolution");
    if evolution_dir.exists() {
        removals.push((evolution_dir, "evolution state"));
    }

    let rc_files = find_rc_files_with_block(&home);
    for rc in &rc_files {
        println!(
            "    {} Harmonia block in {} (shell config)",
            style("*").red(),
            style(rc.display()).dim()
        );
    }

    for (path, label) in &removals {
        println!(
            "    {} {} ({})",
            style("*").red(),
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

    // -- What will NOT be touched --
    println!();
    println!(
        "  {} will {} be touched:",
        style("~/.harmoniis/").cyan(),
        style("NOT").green().bold()
    );
    println!("    vault.db, config.db, metrics.db, config/, frontends/, state/");
    println!("    (Run `harmonia setup` to reconfigure after reinstalling)");
    println!();

    // -- Confirmation --
    let confirmed = Confirm::new()
        .with_prompt(format!(
            "  {} Proceed with uninstall?",
            style("?").yellow().bold()
        ))
        .default(false)
        .interact()?;

    if !confirmed {
        println!("  {} Uninstall cancelled.", style("x").red().bold());
        return Ok(());
    }

    println!();

    // -- Stop daemon --
    // First: kill ALL harmonia processes (not just the PID file one).
    // Multiple runtime/phoenix processes can accumulate from crashes/restarts.
    println!("  {} Stopping all Harmonia processes...", style("->").cyan().bold());
    let _ = std::process::Command::new("pkill").args(["-9", "-f", "harmonia-runtime"]).status();
    let _ = std::process::Command::new("pkill").args(["-9", "-f", "harmonia-phoenix"]).status();
    let _ = std::process::Command::new("pkill").args(["-f", "harmonia node-service"]).status();
    // Then: standard PID-file stop for clean shutdown logging.
    if let Ok(pid_path) = crate::paths::pid_path() {
        if pid_path.exists() {
            let _ = crate::stop::run();
        }
    }

    // -- Remove system service --
    let _ = crate::service::uninstall();

    // -- Remove directories --
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

    // -- Clean shell rc files --
    for rc in &rc_files {
        remove_harmonia_block(rc)?;
        println!(
            "  {} Cleaned Harmonia block from {}",
            style("✓").green().bold(),
            rc.display()
        );
    }

    // -- Legacy cleanup --
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
    let legacy_log = data_dir.join("harmonia.log");
    if legacy_log.exists() {
        fs::remove_file(&legacy_log)?;
    }

    // -- Done --
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
