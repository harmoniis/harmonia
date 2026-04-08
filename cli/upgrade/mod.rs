//! `harmonia upgrade` — version check, download, install with evolution awareness.

mod download;
mod install;
mod version_check;

use console::style;
use std::fs;

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{} Checking for Harmonia updates...",
        style("->").cyan().bold()
    );

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");

    let current_version = VERSION;
    println!("  Current version: {}", style(current_version).yellow());

    let release_info = version_check::fetch_latest_release()?;
    let latest_version = release_info.tag_name.trim_start_matches('v').to_string();
    println!("  Latest version:  {}", style(&latest_version).green());

    if current_version == latest_version {
        println!("\n  {} Already up to date.", style("✓").green().bold());
        return Ok(());
    }

    println!(
        "\n  {} Upgrading {} -> {}",
        style("->").cyan().bold(),
        style(current_version).yellow(),
        style(&latest_version).green()
    );

    let tarball_url = version_check::pick_tarball_url(&release_info)?;
    println!("  Tarball: {}", style(&tarball_url).dim());

    let src_dir = system_dir.join("src");
    let evolution_detected = if src_dir.exists() {
        download::detect_evolution(&src_dir, &system_dir)?
    } else {
        false
    };

    let staging_dir = std::env::temp_dir().join("harmonia-upgrade-staging");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    download::download_and_extract(&tarball_url, &staging_dir)?;
    let extracted_root = download::find_extracted_root(&staging_dir)?;

    if evolution_detected {
        println!(
            "\n  {} Evolution detected — preserving evolved state",
            style("!").yellow().bold()
        );
        install::evolution_upgrade(&extracted_root, &system_dir, &home)?;
    } else {
        println!(
            "\n  {} Clean upgrade (no evolution detected)",
            style("->").cyan().bold()
        );
        install::clean_upgrade(&extracted_root, &system_dir)?;
    }

    let wallet_dir = crate::paths::wallet_root_path()
        .unwrap_or_else(|_| home.join(".harmoniis").join("wallet"));
    let wallet_db = wallet_dir.join("master.db");
    if wallet_dir.exists() || wallet_db.exists() {
        println!("  {} Wallet data untouched", style("✓").green().bold());
    }

    println!("\n  Verifying runtime dependencies...");
    install::check_sbcl_quicklisp(&home)?;

    let _ = fs::remove_dir_all(&staging_dir);

    println!(
        "\n  {} Upgrade to {} complete!",
        style("✓").green().bold(),
        style(&latest_version).green()
    );

    Ok(())
}
