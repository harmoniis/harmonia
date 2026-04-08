//! GitHub release API query and version comparison.

use std::process::Command;

const GITHUB_API_URL: &str = "https://api.github.com/repos/harmoniis/harmonia/releases/latest";

pub(crate) struct ReleaseInfo {
    pub tag_name: String,
    pub assets: Vec<AssetInfo>,
    pub tarball_url: String,
}

pub(crate) struct AssetInfo {
    pub name: String,
    pub browser_download_url: String,
}

pub(crate) fn fetch_latest_release() -> Result<ReleaseInfo, Box<dyn std::error::Error>> {
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

pub(crate) fn pick_tarball_url(
    release: &ReleaseInfo,
) -> Result<String, Box<dyn std::error::Error>> {
    let (os_tag, arch_tag) = platform_tags();

    for asset in &release.assets {
        let lower = asset.name.to_lowercase();
        if lower.contains(&os_tag) && lower.contains(&arch_tag) && lower.ends_with(".tar.gz") {
            return Ok(asset.browser_download_url.clone());
        }
    }
    for asset in &release.assets {
        let lower = asset.name.to_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            return Ok(asset.browser_download_url.clone());
        }
    }
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
