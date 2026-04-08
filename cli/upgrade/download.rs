//! Download, extract, and verify release tarballs.

use console::style;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn download_and_extract(
    url: &str,
    staging_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let tarball_path = staging_dir.join("release.tar.gz");
    println!("  Downloading...");

    let status = Command::new("curl")
        .args(["-sSL", "-o", &tarball_path.to_string_lossy(), url])
        .status()?;

    if !status.success() {
        return Err("failed to download release tarball".into());
    }

    println!("  Extracting...");
    let status = Command::new("tar")
        .args([
            "xzf",
            &tarball_path.to_string_lossy(),
            "-C",
            &staging_dir.to_string_lossy(),
        ])
        .status()?;

    if !status.success() {
        return Err("failed to extract tarball".into());
    }

    Ok(())
}

pub(crate) fn find_extracted_root(
    staging_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    for entry in fs::read_dir(staging_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && entry.file_name() != "." {
            return Ok(path);
        }
    }
    Ok(staging_dir.to_path_buf())
}

pub(crate) fn detect_evolution(
    installed_src: &Path,
    system_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let manifest_path = system_dir.join("install-checksums.sexp");

    if manifest_path.exists() {
        return detect_evolution_via_manifest(installed_src, &manifest_path);
    }

    let has_lisp_files = walk_lisp_files(installed_src)
        .map(|files| !files.is_empty())
        .unwrap_or(false);

    if has_lisp_files {
        println!(
            "    {} No install manifest found — treating Lisp sources as potentially evolved",
            style("!").yellow().bold()
        );
    }

    Ok(has_lisp_files)
}

fn detect_evolution_via_manifest(
    installed_src: &Path,
    manifest_path: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let manifest_body = fs::read_to_string(manifest_path)?;
    let original_checksums = parse_checksum_manifest(&manifest_body);

    let current_files = walk_lisp_files(installed_src)?;
    for file_path in &current_files {
        let rel_path = file_path
            .strip_prefix(installed_src)
            .unwrap_or(file_path)
            .to_string_lossy()
            .to_string();
        let current_hash = sha256_file(file_path)?;
        if let Some(original_hash) = original_checksums.get(&rel_path) {
            if current_hash != *original_hash {
                return Ok(true);
            }
        } else {
            return Ok(true);
        }
    }

    Ok(false)
}

pub(crate) fn parse_checksum_manifest(body: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('(') {
            continue;
        }
        if let Some((hash, path)) = line.split_once("  ") {
            map.insert(path.to_string(), hash.to_string());
        }
    }
    map
}

pub(crate) fn walk_lisp_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    if !dir.exists() {
        return Ok(results);
    }
    walk_lisp_files_recursive(dir, &mut results)?;
    results.sort();
    Ok(results)
}

fn walk_lisp_files_recursive(
    dir: &Path,
    results: &mut Vec<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            walk_lisp_files_recursive(&path, results)?;
        } else if let Some(ext) = path.extension() {
            if ext == "lisp" || ext == "lsp" || ext == "cl" || ext == "asd" {
                results.push(path);
            }
        }
    }
    Ok(())
}

pub(crate) fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("shasum")
        .args(["-a", "256", &path.to_string_lossy()])
        .output()?;
    if !output.status.success() {
        return Err(format!("shasum failed for {}", path.display()).into());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let hash = stdout.split_whitespace().next().unwrap_or("").to_string();
    Ok(hash)
}

pub(crate) fn collect_checksums(
    dir: &Path,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut map = HashMap::new();
    let files = walk_lisp_files(dir)?;
    for file_path in files {
        let rel = file_path
            .strip_prefix(dir)
            .unwrap_or(&file_path)
            .to_string_lossy()
            .to_string();
        let hash = sha256_file(&file_path)?;
        map.insert(rel, hash);
    }
    Ok(map)
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

pub(crate) fn find_src_in_extracted(extracted_root: &Path) -> PathBuf {
    let direct = extracted_root.join("src");
    if direct.join("core").join("boot.lisp").exists() {
        return direct;
    }
    if extracted_root.join("core").join("boot.lisp").exists() {
        return extracted_root.to_path_buf();
    }
    direct
}
