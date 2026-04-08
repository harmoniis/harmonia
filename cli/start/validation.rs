//! Environment checks and path resolution for `harmonia start`.

use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn check_command(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub(crate) fn is_runtime_root(path: &Path) -> bool {
    path.join("src").join("core").join("boot.lisp").exists()
}

pub(crate) fn is_installed_binary() -> bool {
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    let installed_bin_dir = home.join(".local").join("bin");
    std::env::current_exe()
        .ok()
        .map(|exe| exe.starts_with(installed_bin_dir))
        .unwrap_or(false)
}

fn is_truthy_config(raw: &str) -> bool {
    let value = raw.trim();
    !value.is_empty()
        && !matches!(
            value.to_ascii_lowercase().as_str(),
            "0" | "false" | "nil" | "no" | "off"
        )
}

fn source_rewrite_enabled() -> bool {
    harmonia_config_store::get_config("harmonia-cli", "evolution", "source-rewrite-enabled")
        .ok()
        .flatten()
        .map(|raw| is_truthy_config(&raw))
        .unwrap_or(false)
}

pub(crate) fn resolve_source_dir(
    system_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(explicit) = std::env::var("HARMONIA_SOURCE_DIR") {
        let p = PathBuf::from(explicit);
        if is_runtime_root(&p) {
            return Ok(p);
        }
    }

    let stored_source = harmonia_config_store::get_config("harmonia-cli", "global", "source-dir")
        .ok()
        .flatten()
        .map(PathBuf::from)
        .filter(|path| is_runtime_root(path));
    let installed_share = crate::paths::share_dir()
        .ok()
        .filter(|path| is_runtime_root(path));

    if is_installed_binary() && !source_rewrite_enabled() {
        if let Some(share) = installed_share.clone() {
            return Ok(share);
        }
    }

    if let Some(stored) = stored_source {
        return Ok(stored);
    }

    let cwd = std::env::current_dir()?;
    if is_runtime_root(&cwd) {
        return Ok(cwd);
    }

    if let Some(share) = installed_share {
        return Ok(share);
    }

    if is_runtime_root(system_dir) {
        return Ok(system_dir.to_path_buf());
    }

    let exe = std::env::current_exe()?;
    let mut dir = exe.parent().unwrap().to_path_buf();
    for _ in 0..10 {
        if is_runtime_root(&dir) {
            return Ok(dir);
        }
        if let Some(parent) = dir.parent() {
            dir = parent.to_path_buf();
        } else {
            break;
        }
    }

    Err("cannot find Harmonia source directory — run `harmonia setup` first".into())
}

pub(crate) fn resolve_lib_dir(source_dir: &Path) -> PathBuf {
    if let Ok(Some(stored)) =
        harmonia_config_store::get_config("harmonia-cli", "global", "lib-dir")
    {
        let p = PathBuf::from(&stored);
        if p.exists() {
            return p;
        }
    }
    if let Ok(platform_lib) = crate::paths::lib_dir() {
        if platform_lib.exists()
            && platform_lib
                .read_dir()
                .map_or(false, |mut d| d.next().is_some())
        {
            return platform_lib;
        }
    }
    let candidate_target = source_dir.join("target").join("release");
    if candidate_target.exists() {
        return candidate_target;
    }
    candidate_target
}

pub(crate) fn find_phoenix_binary(
    source_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(exe) = std::env::current_exe() {
        let sibling = exe.with_file_name("harmonia-phoenix");
        if sibling.exists() {
            return Ok(sibling);
        }
    }
    if check_command("harmonia-phoenix") {
        return Ok(PathBuf::from("harmonia-phoenix"));
    }
    let dev = source_dir.join("target").join("release").join("phoenix");
    if dev.exists() {
        return Ok(dev);
    }
    Err("harmonia-phoenix binary not found — run install script".into())
}

pub(crate) fn find_sibling_binary(phoenix_bin: &Path, name: &str) -> String {
    if let Some(dir) = phoenix_bin.parent() {
        let sibling = dir.join(name);
        if sibling.exists() {
            return sibling.to_string_lossy().into();
        }
    }
    name.to_string()
}
