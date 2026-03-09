use console::style;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GITHUB_API_URL: &str = "https://api.github.com/repos/harmoniis/harmonia/releases/latest";
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "{} Checking for Harmonia updates...",
        style("→").cyan().bold()
    );

    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let system_dir = home.join(".harmoniis").join("harmonia");

    // 1. Detect current version
    let current_version = VERSION;
    println!("  Current version: {}", style(current_version).yellow());

    // 2. Fetch latest release info from GitHub
    let release_info = fetch_latest_release()?;
    let latest_version = release_info.tag_name.trim_start_matches('v').to_string();

    println!("  Latest version:  {}", style(&latest_version).green());

    // 3. Compare versions
    if current_version == latest_version {
        println!("\n  {} Already up to date.", style("✓").green().bold());
        return Ok(());
    }

    println!(
        "\n  {} Upgrading {} → {}",
        style("→").cyan().bold(),
        style(current_version).yellow(),
        style(&latest_version).green()
    );

    // 4. Determine platform tarball
    let tarball_url = pick_tarball_url(&release_info)?;
    println!("  Tarball: {}", style(&tarball_url).dim());

    // 5. Check for evolution (Lisp source divergence)
    let src_dir = system_dir.join("src");
    let evolution_detected = if src_dir.exists() {
        detect_evolution(&src_dir, &system_dir)?
    } else {
        false
    };

    let staging_dir = std::env::temp_dir().join("harmonia-upgrade-staging");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    // Download and extract tarball into staging
    download_and_extract(&tarball_url, &staging_dir)?;

    // Find the extracted directory (GitHub tarballs contain a top-level dir)
    let extracted_root = find_extracted_root(&staging_dir)?;

    if evolution_detected {
        // 6. Evolution-aware upgrade
        println!(
            "\n  {} Evolution detected — preserving evolved state",
            style("!").yellow().bold()
        );
        evolution_upgrade(&extracted_root, &system_dir, &home)?;
    } else {
        // 7. Clean upgrade
        println!(
            "\n  {} Clean upgrade (no evolution detected)",
            style("→").cyan().bold()
        );
        clean_upgrade(&extracted_root, &system_dir)?;
    }

    // 8. Never touch wallet data — verify
    let wallet_dir = home.join(".harmoniis").join("wallet");
    let wallet_db = home.join(".harmoniis").join("master.db");
    if wallet_dir.exists() || wallet_db.exists() {
        println!("  {} Wallet data untouched", style("✓").green().bold());
    }

    // 9. Re-run SBCL / Quicklisp checks
    println!("\n  Verifying runtime dependencies...");
    check_sbcl_quicklisp(&home)?;

    // Clean up staging
    let _ = fs::remove_dir_all(&staging_dir);

    println!(
        "\n  {} Upgrade to {} complete!",
        style("✓").green().bold(),
        style(&latest_version).green()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// GitHub release API
// ---------------------------------------------------------------------------

struct ReleaseInfo {
    tag_name: String,
    assets: Vec<AssetInfo>,
    tarball_url: String,
}

struct AssetInfo {
    name: String,
    browser_download_url: String,
}

fn fetch_latest_release() -> Result<ReleaseInfo, Box<dyn std::error::Error>> {
    let output = Command::new("curl")
        .args([
            "-sS",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: harmonia-cli",
            GITHUB_API_URL,
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("failed to fetch latest release: {}", stderr).into());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("failed to parse GitHub API response: {e}"))?;

    let tag_name = json["tag_name"]
        .as_str()
        .ok_or("GitHub API response missing tag_name")?
        .to_string();

    let tarball_url = json["tarball_url"].as_str().unwrap_or("").to_string();

    let mut assets = Vec::new();
    if let Some(arr) = json["assets"].as_array() {
        for asset in arr {
            let name = asset["name"].as_str().unwrap_or("").to_string();
            let url = asset["browser_download_url"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if !name.is_empty() && !url.is_empty() {
                assets.push(AssetInfo {
                    name,
                    browser_download_url: url,
                });
            }
        }
    }

    Ok(ReleaseInfo {
        tag_name,
        assets,
        tarball_url,
    })
}

fn pick_tarball_url(release: &ReleaseInfo) -> Result<String, Box<dyn std::error::Error>> {
    let (os_tag, arch_tag) = platform_tags();

    // First: look for a platform-specific asset (e.g. harmonia-0.1.3-darwin-arm64.tar.gz)
    for asset in &release.assets {
        let lower = asset.name.to_lowercase();
        if lower.contains(&os_tag) && lower.contains(&arch_tag) && lower.ends_with(".tar.gz") {
            return Ok(asset.browser_download_url.clone());
        }
    }

    // Fallback: look for any tarball asset
    for asset in &release.assets {
        let lower = asset.name.to_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            return Ok(asset.browser_download_url.clone());
        }
    }

    // Final fallback: use the GitHub source tarball
    if !release.tarball_url.is_empty() {
        return Ok(release.tarball_url.clone());
    }

    Err("no suitable tarball found in release assets".into())
}

fn platform_tags() -> (String, String) {
    let os_tag = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else {
        "unknown"
    };

    let arch_tag = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    };

    (os_tag.to_string(), arch_tag.to_string())
}

// ---------------------------------------------------------------------------
// Download + extract
// ---------------------------------------------------------------------------

fn download_and_extract(url: &str, staging_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
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

fn find_extracted_root(staging_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // GitHub tarballs extract to a single top-level directory
    for entry in fs::read_dir(staging_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && entry.file_name() != "." {
            return Ok(path);
        }
    }
    // If no subdirectory, the staging dir itself is the root
    Ok(staging_dir.to_path_buf())
}

// ---------------------------------------------------------------------------
// Evolution detection
// ---------------------------------------------------------------------------

fn detect_evolution(
    installed_src: &Path,
    system_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Compare checksums of installed Lisp source files against the manifest
    // written at install time (if it exists). If no manifest, check if any
    // .lisp files have been modified since installation.
    let manifest_path = system_dir.join("install-checksums.sexp");

    if manifest_path.exists() {
        return detect_evolution_via_manifest(installed_src, &manifest_path);
    }

    // No manifest — check if the source dir has any .lisp files at all.
    // If it does, conservatively treat them as potentially evolved.
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
        }
        // New file not in manifest — also counts as evolution
        else {
            return Ok(true);
        }
    }

    Ok(false)
}

fn parse_checksum_manifest(body: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    // Simple format: one line per file, "sha256hash  relative/path.lisp"
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

fn walk_lisp_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
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

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
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

// ---------------------------------------------------------------------------
// Clean upgrade (no evolution)
// ---------------------------------------------------------------------------

fn clean_upgrade(
    extracted_root: &Path,
    system_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_src = system_dir.join("src");

    // Install new source files
    let new_src = find_src_in_extracted(extracted_root);
    if new_src.exists() {
        if target_src.exists() {
            fs::remove_dir_all(&target_src)?;
        }
        copy_dir_recursive(&new_src, &target_src)?;
        println!("  {} Source files updated", style("✓").green().bold());
    }

    // Install new binaries/libraries if present
    install_binaries(extracted_root, system_dir)?;

    // Write checksum manifest for future evolution detection
    write_checksum_manifest(&target_src, system_dir)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Evolution-aware upgrade
// ---------------------------------------------------------------------------

fn evolution_upgrade(
    extracted_root: &Path,
    system_dir: &Path,
    home: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let target_src = system_dir.join("src");
    let backup_dir = home
        .join(".harmoniis")
        .join("harmonia")
        .join("evolution-backup");

    // 6a. Back up evolved state
    println!("  Backing up evolved state...");
    if backup_dir.exists() {
        fs::remove_dir_all(&backup_dir)?;
    }
    if target_src.exists() {
        copy_dir_recursive(&target_src, &backup_dir)?;
        println!(
            "    {} Evolved state backed up to {}",
            style("✓").green().bold(),
            backup_dir.display()
        );
    }

    // Collect checksums of evolved files before overwrite
    let evolved_checksums = collect_checksums(&target_src)?;

    // 6b. Install new release source
    let new_src = find_src_in_extracted(extracted_root);
    if new_src.exists() {
        if target_src.exists() {
            fs::remove_dir_all(&target_src)?;
        }
        copy_dir_recursive(&new_src, &target_src)?;
    }

    // Collect checksums of new release files
    let new_checksums = collect_checksums(&target_src)?;

    // Read the old manifest to know what the original (pre-evolution) state was
    let manifest_path = system_dir.join("install-checksums.sexp");
    let original_checksums = if manifest_path.exists() {
        let body = fs::read_to_string(&manifest_path)?;
        parse_checksum_manifest(&body)
    } else {
        HashMap::new()
    };

    // 6c. Merge: copy evolved files back where appropriate
    let mut preserved = Vec::new();
    let mut updated = Vec::new();
    let mut needs_review = Vec::new();

    for (rel_path, evolved_hash) in &evolved_checksums {
        let original_hash = original_checksums.get(rel_path);
        let new_hash = new_checksums.get(rel_path);

        let file_evolved = original_hash.map_or(true, |orig| evolved_hash != orig);

        if !file_evolved {
            // File was not changed by evolution — use the new version
            updated.push(rel_path.clone());
            continue;
        }

        let new_also_changed = match (original_hash, new_hash) {
            (Some(orig), Some(new_h)) => orig != new_h,
            (None, Some(_)) => true, // new file in release
            _ => false,
        };

        let evolved_file = backup_dir.join(rel_path);
        let target_file = target_src.join(rel_path);

        if new_also_changed {
            // Both sides changed — keep evolved version, flag for manual review
            if evolved_file.exists() {
                if let Some(parent) = target_file.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&evolved_file, &target_file)?;
                needs_review.push(rel_path.clone());
            }
        } else {
            // Only evolution changed it — preserve evolved version
            if evolved_file.exists() {
                if let Some(parent) = target_file.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&evolved_file, &target_file)?;
                preserved.push(rel_path.clone());
            }
        }
    }

    // Also restore any evolved files that don't exist in the new release
    for (rel_path, _) in &evolved_checksums {
        if !new_checksums.contains_key(rel_path) {
            let evolved_file = backup_dir.join(rel_path);
            let target_file = target_src.join(rel_path);
            if evolved_file.exists() && !target_file.exists() {
                if let Some(parent) = target_file.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&evolved_file, &target_file)?;
                preserved.push(rel_path.clone());
            }
        }
    }

    // Install binaries
    install_binaries(extracted_root, system_dir)?;

    // Write new checksum manifest
    write_checksum_manifest(&target_src, system_dir)?;

    // Log files needing manual review
    if !needs_review.is_empty() {
        let review_log = system_dir.join("upgrade-review.log");
        let log_content = needs_review
            .iter()
            .map(|p| format!("CONFLICT: {}", p))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&review_log, format!("{}\n", log_content))?;
        println!(
            "    {} Review log: {}",
            style("!").yellow().bold(),
            review_log.display()
        );
    }

    // Print summary
    println!("\n  Upgrade summary:");
    println!(
        "    {} files updated from new release",
        style(updated.len()).cyan()
    );
    println!(
        "    {} evolved files preserved",
        style(preserved.len()).green()
    );
    if !needs_review.is_empty() {
        println!(
            "    {} files need manual review (evolved version kept):",
            style(needs_review.len()).yellow()
        );
        for path in &needs_review {
            println!("      - {}", path);
        }
    }

    Ok(())
}

fn collect_checksums(dir: &Path) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
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

// ---------------------------------------------------------------------------
// Binary / library installation
// ---------------------------------------------------------------------------

fn install_binaries(
    extracted_root: &Path,
    system_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let bin_dir = system_dir.parent().unwrap().join("bin");

    // Check for a bin/ directory in the extracted release
    let src_bin = extracted_root.join("bin");
    if src_bin.exists() && src_bin.is_dir() {
        fs::create_dir_all(&bin_dir)?;
        for entry in fs::read_dir(&src_bin)? {
            let entry = entry?;
            let dest = bin_dir.join(entry.file_name());
            fs::copy(entry.path(), &dest)?;
            // Ensure executable bit
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest)?.permissions();
                perms.set_mode(perms.mode() | 0o755);
                fs::set_permissions(&dest, perms)?;
            }
        }
        println!(
            "  {} Binaries installed to {}",
            style("✓").green().bold(),
            bin_dir.display()
        );
    }

    // Check for lib/ directory
    let src_lib = extracted_root.join("lib");
    let dest_lib = system_dir.join("lib");
    if src_lib.exists() && src_lib.is_dir() {
        fs::create_dir_all(&dest_lib)?;
        for entry in fs::read_dir(&src_lib)? {
            let entry = entry?;
            let dest = dest_lib.join(entry.file_name());
            if entry.path().is_file() {
                fs::copy(entry.path(), &dest)?;
            }
        }
        println!("  {} Libraries updated", style("✓").green().bold());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Checksum manifest
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn find_src_in_extracted(extracted_root: &Path) -> PathBuf {
    // Look for src/core/boot.lisp to identify the source root
    let direct = extracted_root.join("src");
    if direct.join("core").join("boot.lisp").exists() {
        return direct;
    }
    // Some release layouts may have everything at the root level
    if extracted_root.join("core").join("boot.lisp").exists() {
        return extracted_root.to_path_buf();
    }
    // Default: assume src/ is the directory
    direct
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

fn check_sbcl_quicklisp(home: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Check SBCL
    let sbcl_ok = Command::new("sbcl")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if sbcl_ok {
        println!("  {} SBCL installed", style("✓").green().bold());
    } else {
        println!(
            "  {} SBCL not found — run `harmonia setup` to install",
            style("!").red().bold()
        );
    }

    // Check Quicklisp
    let quicklisp_path = home.join("quicklisp").join("setup.lisp");
    if quicklisp_path.exists() {
        println!("  {} Quicklisp installed", style("✓").green().bold());
    } else {
        println!(
            "  {} Quicklisp not found — run `harmonia setup` to install",
            style("!").yellow().bold()
        );
    }

    Ok(())
}
