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

pub(crate) fn resolve_source_dir(
    system_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    crate::paths::resolve_runtime_root(system_dir)
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
    // Check sibling of current exe (both names)
    if let Ok(exe) = std::env::current_exe() {
        for name in ["harmonia-phoenix", "phoenix"] {
            let sibling = exe.with_file_name(name);
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }
    // Check dev build paths (debug + release, both names)
    for profile in ["debug", "release"] {
        for name in ["phoenix", "harmonia-phoenix"] {
            let dev = source_dir.join("target").join(profile).join(name);
            if dev.exists() {
                return Ok(dev);
            }
        }
    }
    // Check installed lib dir
    if let Ok(lib) = crate::paths::lib_dir() {
        let installed = lib.join("phoenix");
        if installed.exists() {
            return Ok(installed);
        }
    }
    // Last resort: PATH (may block if binary hangs on --version)
    if check_command("harmonia-phoenix") {
        return Ok(PathBuf::from("harmonia-phoenix"));
    }
    Err("harmonia-phoenix binary not found — run install script".into())
}

pub(crate) fn find_sibling_binary(phoenix_bin: &Path, name: &str) -> String {
    // Check sibling of phoenix binary
    if let Some(dir) = phoenix_bin.parent() {
        let sibling = dir.join(name);
        if sibling.exists() {
            return sibling.to_string_lossy().into();
        }
    }
    // Check installed lib dir
    if let Ok(lib) = crate::paths::lib_dir() {
        let installed = lib.join(name);
        if installed.exists() {
            return installed.to_string_lossy().into();
        }
    }
    name.to_string()
}
