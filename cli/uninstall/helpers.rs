//! Shared helpers for uninstall: filesystem, timestamps, shell rc cleanup.

use std::fs;
use std::path::{Path, PathBuf};

const SHELL_RC_FILES: &[&str] = &[".zshrc", ".bashrc"];
const BLOCK_START: &str = "# Harmonia agent";

pub(crate) fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}", now)
}

pub(crate) fn find_extracted_root(staging: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for entry in fs::read_dir(staging)? {
        let entry = entry?;
        if entry.path().is_dir() {
            return Ok(entry.path());
        }
    }
    Ok(staging.to_path_buf())
}

pub(crate) fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub(crate) fn find_rc_files_with_block(home: &Path) -> Vec<PathBuf> {
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

pub(crate) fn merge_evolution_dirs(
    src: &Path,
    dst: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub(crate) fn remove_harmonia_block(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
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
