//! Shared helper functions for the setup module.

use console::Term;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn check_command(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Read a secret with per-character `*` feedback (handles typing, paste, and backspace).
pub(crate) fn read_masked(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    let term = Term::stderr();
    eprint!("{}: ", prompt);
    std::io::stderr().flush()?;
    let mut buf = String::new();
    loop {
        let key = term.read_key()?;
        match key {
            console::Key::Char(c) if !c.is_control() => {
                buf.push(c);
                eprint!("*");
                std::io::stderr().flush()?;
            }
            console::Key::Backspace => {
                if buf.pop().is_some() {
                    eprint!("\x08 \x08");
                    std::io::stderr().flush()?;
                }
            }
            console::Key::Enter => {
                eprintln!();
                break;
            }
            _ => {}
        }
    }
    Ok(buf)
}

pub(crate) fn ensure_harmoniis_wallet() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let wallet_root = crate::paths::wallet_root_path()?;
    let wallet_path = wallet_root.join("master.db");
    if wallet_path.exists() || wallet_root.join("rgb.db").exists() {
        return Ok(wallet_path);
    }

    ensure_wallet_cli()?;

    let wallet_path_string = wallet_path.to_string_lossy().to_string();

    let mut output = Command::new("hrmw")
        .args([
            "setup",
            "--wallet",
            &wallet_path_string,
            "--password-manager",
            "best-effort",
        ])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if stderr.contains("unexpected argument '--password-manager'") {
            output = Command::new("hrmw")
                .args(["setup", "--wallet", &wallet_path_string])
                .output()?;
        }
    }

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("failed to initialize harmoniis-wallet: {stderr}").into());
    }

    if !wallet_path.exists() {
        return Err(format!(
            "harmoniis-wallet setup completed but wallet file not found at {}",
            wallet_path.display()
        )
        .into());
    }
    Ok(wallet_path)
}

fn ensure_wallet_cli() -> Result<(), Box<dyn std::error::Error>> {
    const REQUIRED: (u64, u64, u64) = (0, 1, 26);

    let needs_install = match installed_hrmw_version() {
        Some(v) => v < REQUIRED,
        None => true,
    };
    if !needs_install {
        return Ok(());
    }

    if !check_command("cargo") {
        return Err(
            "cargo is required to install harmoniis-wallet (hrmw) for vault bootstrap".into(),
        );
    }

    println!(
        "    {} Installing harmoniis-wallet CLI (hrmw >= {}.{}.{})...",
        console::style("->").cyan().bold(),
        REQUIRED.0,
        REQUIRED.1,
        REQUIRED.2
    );
    let status = Command::new("cargo")
        .args([
            "install",
            "harmoniis-wallet",
            "--version",
            "0.1.26",
            "--force",
        ])
        .status()?;
    if !status.success() {
        return Err("failed to install harmoniis-wallet CLI".into());
    }
    Ok(())
}

fn installed_hrmw_version() -> Option<(u64, u64, u64)> {
    let output = Command::new("hrmw").arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let version = text.split_whitespace().nth(1)?;
    parse_semver_tuple(version)
}

fn parse_semver_tuple(input: &str) -> Option<(u64, u64, u64)> {
    let clean = input
        .trim()
        .split('-')
        .next()
        .unwrap_or(input)
        .trim_start_matches('v');
    let mut parts = clean.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor, patch))
}

pub(crate) fn install_quicklisp() -> Result<(), Box<dyn std::error::Error>> {
    let tmp = std::env::temp_dir().join("quicklisp.lisp");
    let output = Command::new("curl")
        .args([
            "-sS",
            "-o",
            &tmp.to_string_lossy(),
            "https://beta.quicklisp.org/quicklisp.lisp",
        ])
        .output()?;

    if !output.status.success() {
        return Err("failed to download quicklisp.lisp".into());
    }

    let output = Command::new("sbcl")
        .arg("--non-interactive")
        .arg("--load")
        .arg(&tmp)
        .arg("--eval")
        .arg("(quicklisp-quickstart:install)")
        .arg("--eval")
        .arg("(ql:add-to-init-file)")
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("quicklisp install failed: {}", stderr).into());
    }

    Ok(())
}

pub(crate) fn read_existing_workspace(system_dir: &Path) -> Option<String> {
    let ws_path = system_dir.join("config").join("workspace.sexp");
    let content = fs::read_to_string(ws_path).ok()?;
    let marker = ":user-workspace \"";
    let start = content.find(marker)? + marker.len();
    let end = content[start..].find('"')? + start;
    let path = &content[start..end];
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

pub(crate) fn find_source_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let system_dir = crate::paths::data_dir()?;
    crate::paths::resolve_runtime_root(&system_dir)
}

pub(crate) fn copy_dir_recursive(
    src: &PathBuf,
    dst: &PathBuf,
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

pub(crate) fn install_cdylibs(
    target_dir: &Path,
    lib_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let ext = if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "windows") {
        "dll"
    } else {
        "so"
    };
    let prefix = if cfg!(target_os = "windows") {
        ""
    } else {
        "lib"
    };
    fs::create_dir_all(lib_dir)?;
    for entry in fs::read_dir(target_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.contains("harmonia") && name.ends_with(ext) {
            let dest = lib_dir.join(&name);
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}
