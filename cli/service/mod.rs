mod platform;

use std::path::PathBuf;

pub fn install() -> Result<(), Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    let harmonia_bin = resolve_bin_path()?;
    platform::install(&home, &harmonia_bin)
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    platform::uninstall()
}

fn resolve_bin_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let canonical = std::fs::canonicalize(&exe).unwrap_or(exe);
    Ok(canonical)
}
