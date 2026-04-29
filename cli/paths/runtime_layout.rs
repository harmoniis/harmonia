//! Canonical resolution of the Harmonia Lisp runtime root (`boot.lisp` tree).
//!
//! Same rules as service startup (`harmonia start`): env override, stored config,
//! installed share tree, cwd, legacy data dir as tree, walk up from exe.

use std::path::{Path, PathBuf};

pub fn is_runtime_root(path: &Path) -> bool {
    path.join("src").join("core").join("boot.lisp").exists()
}

/// True when the binary lives under a typical user-level install prefix (not a repo `target/`).
pub fn is_standard_user_bin_install() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(home) = dirs::home_dir() else {
        return false;
    };
    exe.starts_with(home.join(".local").join("bin"))
        || exe.starts_with(home.join(".cargo").join("bin"))
}

fn source_rewrite_enabled() -> bool {
    harmonia_config_store::get_config("harmonia-cli", "evolution", "source-rewrite-enabled")
        .ok()
        .flatten()
        .map(|raw| {
            let v = raw.trim();
            !v.is_empty()
                && !matches!(
                    v.to_ascii_lowercase().as_str(),
                    "0" | "false" | "nil" | "no" | "off"
                )
        })
        .unwrap_or(false)
}

pub fn resolve_runtime_root(system_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
    let installed_share = super::share_dir()
        .ok()
        .filter(|path| is_runtime_root(path));

    if is_standard_user_bin_install() && !source_rewrite_enabled() {
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

    Err("cannot find Harmonia source directory — run `harmonia setup` from the project repo or use `scripts/install.sh`; for a dev checkout set HARMONIA_SOURCE_DIR"
        .into())
}
