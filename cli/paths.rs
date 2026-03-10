use std::path::PathBuf;

/// User data: ~/.harmoniis/harmonia/
/// Contains config.db, vault.db, config/, state/, frontends/ — nothing else.
pub fn data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    Ok(home.join(".harmoniis").join("harmonia"))
}

/// Application libraries (cdylibs): ~/.local/lib/harmonia/
/// Platform-standard location for user-installed shared libraries.
pub fn lib_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = platform_lib_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Application data (source, docs, genesis): ~/.local/share/harmonia/
/// Platform-standard location for user-installed application data.
pub fn share_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = platform_share_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Installed Lisp source tree: ~/.local/share/harmonia/src/
pub fn source_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(share_dir()?.join("src"))
}

/// Runtime directory for PID files and sockets.
///   macOS:   $TMPDIR/harmonia/
///   Linux:   $XDG_RUNTIME_DIR/harmonia/  (fallback: /tmp/harmonia-$UID/)
///   Other:   /tmp/harmonia/
pub fn run_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = platform_run_dir();
    std::fs::create_dir_all(&dir)?;
    // Owner-only permissions on the runtime dir
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Log directory.
///   macOS:   ~/Library/Logs/Harmonia/
///   Linux:   $XDG_STATE_HOME/harmonia/  (fallback: ~/.local/state/harmonia/)
///   Other:   ~/.local/state/harmonia/
pub fn log_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = platform_log_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn pid_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia.pid"))
}

pub fn broker_pid_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia-mqtt-broker.pid"))
}

pub fn socket_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia.sock"))
}

pub fn log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(log_dir()?.join("harmonia.log"))
}

pub fn broker_log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(log_dir()?.join("harmonia-mqtt-broker.log"))
}

// --- Platform-specific resolution ---

// --- Library and share dirs (XDG-style, all platforms) ---

#[cfg(target_os = "macos")]
fn platform_lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "macos")]
fn platform_share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "linux")]
fn platform_lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "linux")]
fn platform_share_dir() -> PathBuf {
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(data).join("harmonia")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "freebsd")]
fn platform_lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "freebsd")]
fn platform_share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "windows")]
fn platform_lib_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("Harmonia").join("lib")
    } else if let Some(home) = dirs::home_dir() {
        home.join("AppData")
            .join("Local")
            .join("Harmonia")
            .join("lib")
    } else {
        PathBuf::from("C:\\ProgramData\\Harmonia\\lib")
    }
}

#[cfg(target_os = "windows")]
fn platform_share_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("Harmonia").join("share")
    } else if let Some(home) = dirs::home_dir() {
        home.join("AppData")
            .join("Local")
            .join("Harmonia")
            .join("share")
    } else {
        PathBuf::from("C:\\ProgramData\\Harmonia\\share")
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
fn platform_lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
fn platform_share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

// --- Runtime and log dirs (platform-specific) ---

#[cfg(target_os = "macos")]
fn platform_run_dir() -> PathBuf {
    // $TMPDIR is per-user on macOS (e.g. /var/folders/xx/.../T/)
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        PathBuf::from(tmpdir).join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia")
    }
}

#[cfg(target_os = "macos")]
fn platform_log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join("Library").join("Logs").join("Harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "linux")]
fn platform_run_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(xdg).join("harmonia")
    } else {
        // Fallback: /tmp/harmonia-UID/
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/harmonia-{}", uid))
    }
}

#[cfg(target_os = "linux")]
fn platform_log_dir() -> PathBuf {
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(state).join("harmonia")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "freebsd")]
fn platform_run_dir() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/harmonia-{}", uid))
}

#[cfg(target_os = "freebsd")]
fn platform_log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "windows")]
fn platform_run_dir() -> PathBuf {
    // %LOCALAPPDATA%\Harmonia\run\
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("Harmonia").join("run")
    } else if let Some(home) = dirs::home_dir() {
        home.join("AppData")
            .join("Local")
            .join("Harmonia")
            .join("run")
    } else {
        PathBuf::from("C:\\ProgramData\\Harmonia\\run")
    }
}

#[cfg(target_os = "windows")]
fn platform_log_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        PathBuf::from(local).join("Harmonia").join("Logs")
    } else if let Some(home) = dirs::home_dir() {
        home.join("AppData")
            .join("Local")
            .join("Harmonia")
            .join("Logs")
    } else {
        PathBuf::from("C:\\ProgramData\\Harmonia\\Logs")
    }
}

// Catch-all for other platforms
#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
fn platform_run_dir() -> PathBuf {
    PathBuf::from("/tmp/harmonia")
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
fn platform_log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}
