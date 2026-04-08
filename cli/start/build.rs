//! Runtime artifact build and verification.

use console::style;
use std::path::Path;
use std::process::Command;

use super::validation::{check_command, resolve_lib_dir};

fn required_runtime_libraries() -> Vec<String> {
    Vec::new()
}

fn missing_runtime_artifacts(lib_dir: &Path) -> Vec<String> {
    required_runtime_libraries()
        .into_iter()
        .filter(|name| !lib_dir.join(name).exists())
        .collect()
}

pub(crate) fn ensure_runtime_artifacts(
    source_dir: &Path,
    lib_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let missing = missing_runtime_artifacts(lib_dir);
    if missing.is_empty() {
        return Ok(());
    }

    let has_source_build = source_dir.join("Cargo.toml").exists();
    if !has_source_build {
        return Err(format!(
            "missing runtime libraries in {}: {}",
            lib_dir.display(),
            missing.join(", ")
        )
        .into());
    }

    if !check_command("cargo") {
        return Err("cargo is required to build missing runtime artifacts".into());
    }

    println!(
        "{} Missing runtime libraries ({}). Building release workspace...",
        style("->").cyan().bold(),
        missing.len()
    );
    let status = Command::new("cargo")
        .args(["build", "--workspace", "--release"])
        .current_dir(source_dir)
        .status()?;
    if !status.success() {
        return Err("failed to build release runtime artifacts".into());
    }

    let rebuilt_lib_dir = resolve_lib_dir(source_dir);
    let after = missing_runtime_artifacts(&rebuilt_lib_dir);
    if !after.is_empty() {
        return Err(format!(
            "runtime libraries still missing after build: {}",
            after.join(", ")
        )
        .into());
    }
    Ok(())
}
