//! Evolution export/import and safety checks for uninstall.

use console::style;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::helpers::{chrono_timestamp, copy_dir_recursive, find_extracted_root, merge_evolution_dirs};

// --- Export ---

pub fn run_evolution_export(output: Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let data_dir = crate::paths::data_dir()?;
    let share_dir = crate::paths::share_dir()?;

    let evolution_dir = data_dir.join("evolution");
    let share_evolution = share_dir.join("genesis");

    if !evolution_dir.exists() && !share_evolution.exists() {
        println!(
            "  {} No evolution state found — nothing to export.",
            style("!").yellow().bold()
        );
        return Ok(());
    }

    let timestamp = chrono_timestamp();
    let default_name = format!("harmonia-evolution-{}.tar.gz", timestamp);
    let output_path = PathBuf::from(
        output.unwrap_or_else(|| home.join(&default_name).to_string_lossy().to_string()),
    );

    println!(
        "  {} Exporting evolution state...",
        style("->").cyan().bold()
    );

    let staging = std::env::temp_dir().join(format!("harmonia-evolution-export-{}", timestamp));
    fs::create_dir_all(&staging)?;

    if evolution_dir.exists() {
        copy_dir_recursive(&evolution_dir, &staging.join("evolution"))?;
    }
    if share_evolution.exists() {
        copy_dir_recursive(&share_evolution, &staging.join("genesis"))?;
    }

    export_config_keys(&staging)?;

    let meta = format!("(:export-version 1\n :timestamp \"{}\"\n :platform \"{}\")\n", timestamp, std::env::consts::OS);
    fs::write(staging.join("manifest.sexp"), &meta)?;

    let status = Command::new("tar")
        .args([
            "-czf",
            &output_path.to_string_lossy(),
            "-C",
            &staging.parent().unwrap().to_string_lossy(),
            &staging.file_name().unwrap().to_string_lossy().to_string(),
        ])
        .status()?;

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

// --- Import ---

pub fn run_evolution_import(path: &str, merge: bool) -> Result<(), Box<dyn std::error::Error>> {
    let archive = PathBuf::from(path);
    if !archive.exists() {
        return Err(format!("archive not found: {}", path).into());
    }

    let data_dir = crate::paths::data_dir()?;
    let share_dir = crate::paths::share_dir()?;

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

    let extracted_root = find_extracted_root(&staging)?;

    let manifest = extracted_root.join("manifest.sexp");
    if !manifest.exists() {
        let _ = fs::remove_dir_all(&staging);
        return Err("invalid evolution archive — missing manifest.sexp".into());
    }

    println!(
        "  {} Importing evolution state{}...",
        style("->").cyan().bold(),
        if merge { " (merge mode)" } else { "" }
    );

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

    let src_genesis = extracted_root.join("genesis");
    if src_genesis.exists() {
        let dst_genesis = share_dir.join("genesis");
        if !merge && dst_genesis.exists() {
            fs::remove_dir_all(&dst_genesis)?;
        }
        copy_dir_recursive(&src_genesis, &dst_genesis)?;
    }

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

// --- Safety checks ---

pub fn read_evolution_version(data_dir: &Path) -> u32 {
    let version_file = data_dir.join("evolution").join("version.sexp");
    if let Ok(content) = fs::read_to_string(&version_file) {
        content.trim().parse::<u32>().unwrap_or(0)
    } else {
        0
    }
}

pub fn check_source_pushed(data_dir: &Path) -> bool {
    if let Ok(Some(source_dir)) =
        harmonia_config_store::get_config("harmonia-cli", "global", "source-dir")
    {
        let source_path = PathBuf::from(&source_dir);
        if source_path.join(".git").exists() {
            let output = Command::new("git")
                .args(["log", "--oneline", "@{u}..HEAD"])
                .current_dir(&source_path)
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let unpushed = String::from_utf8_lossy(&out.stdout);
                    return unpushed.trim().is_empty();
                }
            }
        }
    }

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

pub fn check_distributed_propagated() -> bool {
    if let Ok(Some(enabled)) = harmonia_config_store::get_config(
        "harmonia-cli",
        "evolution",
        "distributed-evolution-enabled",
    ) {
        let is_enabled = matches!(enabled.to_lowercase().as_str(), "1" | "true" | "yes" | "on");
        if !is_enabled {
            return false;
        }
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

// --- Config key export/import ---

fn export_config_keys(staging: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
    let content = fs::read_to_string(path)?;
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

