//! Upgrade installation: clean upgrade, evolution-aware upgrade, binary swap.

use console::style;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::download::{
    collect_checksums, copy_dir_recursive, find_src_in_extracted, parse_checksum_manifest,
};

pub(crate) fn clean_upgrade(
    extracted_root: &Path,
    system_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_src = system_dir.join("src");
    let new_src = find_src_in_extracted(extracted_root);
    if new_src.exists() {
        if target_src.exists() {
            fs::remove_dir_all(&target_src)?;
        }
        copy_dir_recursive(&new_src, &target_src)?;
        println!("  {} Source files updated", style("✓").green().bold());
    }
    install_binaries(extracted_root, system_dir)?;
    write_checksum_manifest(&target_src, system_dir)?;
    Ok(())
}

pub(crate) fn evolution_upgrade(
    extracted_root: &Path,
    system_dir: &Path,
    home: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_src = system_dir.join("src");
    let backup_dir = home
        .join(".harmoniis")
        .join("harmonia")
        .join("evolution-backup");

    println!("  Backing up evolved state...");
    if backup_dir.exists() { fs::remove_dir_all(&backup_dir)?; }
    if target_src.exists() {
        copy_dir_recursive(&target_src, &backup_dir)?;
        println!("    {} Evolved state backed up to {}", style("✓").green().bold(), backup_dir.display());
    }

    let evolved_checksums = collect_checksums(&target_src)?;

    let new_src = find_src_in_extracted(extracted_root);
    if new_src.exists() {
        if target_src.exists() { fs::remove_dir_all(&target_src)?; }
        copy_dir_recursive(&new_src, &target_src)?;
    }

    let new_checksums = collect_checksums(&target_src)?;
    let manifest_path = system_dir.join("install-checksums.sexp");
    let original_checksums = if manifest_path.exists() {
        let body = fs::read_to_string(&manifest_path)?;
        parse_checksum_manifest(&body)
    } else { HashMap::new() };

    let mut preserved = Vec::new();
    let mut updated = Vec::new();
    let mut needs_review = Vec::new();

    for (rel_path, evolved_hash) in &evolved_checksums {
        let original_hash = original_checksums.get(rel_path);
        let new_hash = new_checksums.get(rel_path);
        let file_evolved = original_hash.map_or(true, |orig| evolved_hash != orig);
        if !file_evolved { updated.push(rel_path.clone()); continue; }
        let new_also_changed = match (original_hash, new_hash) {
            (Some(orig), Some(new_h)) => orig != new_h,
            (None, Some(_)) => true,
            _ => false,
        };
        let evolved_file = backup_dir.join(rel_path);
        let target_file = target_src.join(rel_path);
        if new_also_changed {
            if evolved_file.exists() {
                if let Some(parent) = target_file.parent() { fs::create_dir_all(parent)?; }
                fs::copy(&evolved_file, &target_file)?;
                needs_review.push(rel_path.clone());
            }
        } else if evolved_file.exists() {
            if let Some(parent) = target_file.parent() { fs::create_dir_all(parent)?; }
            fs::copy(&evolved_file, &target_file)?;
            preserved.push(rel_path.clone());
        }
    }

    for (rel_path, _) in &evolved_checksums {
        if !new_checksums.contains_key(rel_path) {
            let evolved_file = backup_dir.join(rel_path);
            let target_file = target_src.join(rel_path);
            if evolved_file.exists() && !target_file.exists() {
                if let Some(parent) = target_file.parent() { fs::create_dir_all(parent)?; }
                fs::copy(&evolved_file, &target_file)?;
                preserved.push(rel_path.clone());
            }
        }
    }

    install_binaries(extracted_root, system_dir)?;
    write_checksum_manifest(&target_src, system_dir)?;

    if !needs_review.is_empty() {
        let review_log = system_dir.join("upgrade-review.log");
        let log_content = needs_review.iter().map(|p| format!("CONFLICT: {}", p)).collect::<Vec<_>>().join("\n");
        fs::write(&review_log, format!("{}\n", log_content))?;
        println!("    {} Review log: {}", style("!").yellow().bold(), review_log.display());
    }

    println!("\n  Upgrade summary:");
    println!("    {} files updated from new release", style(updated.len()).cyan());
    println!("    {} evolved files preserved", style(preserved.len()).green());
    if !needs_review.is_empty() {
        println!("    {} files need manual review (evolved version kept):", style(needs_review.len()).yellow());
        for path in &needs_review { println!("      - {}", path); }
    }

    Ok(())
}

fn install_binaries(
    extracted_root: &Path,
    system_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bin_dir = system_dir.parent().unwrap().join("bin");
    let src_bin = extracted_root.join("bin");
    if src_bin.exists() && src_bin.is_dir() {
        fs::create_dir_all(&bin_dir)?;
        for entry in fs::read_dir(&src_bin)? {
            let entry = entry?;
            let dest = bin_dir.join(entry.file_name());
            fs::copy(entry.path(), &dest)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest)?.permissions();
                perms.set_mode(perms.mode() | 0o755);
                fs::set_permissions(&dest, perms)?;
            }
        }
        println!("  {} Binaries installed to {}", style("✓").green().bold(), bin_dir.display());
    }

    let src_lib = extracted_root.join("lib");
    let dest_lib = system_dir.join("lib");
    if src_lib.exists() && src_lib.is_dir() {
        fs::create_dir_all(&dest_lib)?;
        for entry in fs::read_dir(&src_lib)? {
            let entry = entry?;
            let dest = dest_lib.join(entry.file_name());
            if entry.path().is_file() { fs::copy(entry.path(), &dest)?; }
        }
        println!("  {} Libraries updated", style("✓").green().bold());
    }
    Ok(())
}

fn write_checksum_manifest(
    src_dir: &Path,
    system_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let checksums = collect_checksums(src_dir)?;
    let manifest_path = system_dir.join("install-checksums.sexp");
    let mut lines: Vec<String> = vec![
        ";; Harmonia install checksums — auto-generated by `harmonia upgrade`".to_string(),
        ";; Format: sha256hash  relative/path".to_string(),
    ];
    let mut sorted: Vec<_> = checksums.iter().collect();
    sorted.sort_by_key(|(path, _)| (*path).clone());
    for (path, hash) in sorted {
        lines.push(format!("{}  {}", hash, path));
    }
    fs::write(&manifest_path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

pub(crate) fn check_sbcl_quicklisp(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sbcl_ok = Command::new("sbcl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if sbcl_ok {
        println!("  {} SBCL installed", style("✓").green().bold());
    } else {
        println!("  {} SBCL not found — run `harmonia setup` to install", style("!").red().bold());
    }

    let quicklisp_path = home.join("quicklisp").join("setup.lisp");
    if quicklisp_path.exists() {
        println!("  {} Quicklisp installed", style("✓").green().bold());
    } else {
        println!("  {} Quicklisp not found — run `harmonia setup` to install", style("!").yellow().bold());
    }

    Ok(())
}
