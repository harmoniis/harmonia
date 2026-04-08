use std::path::PathBuf;

// --- Library and share dirs (XDG-style, all platforms) ---

#[cfg(target_os = "macos")]
pub(super) fn lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "macos")]
pub(super) fn share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "linux")]
pub(super) fn lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "linux")]
pub(super) fn share_dir() -> PathBuf {
    if let Ok(data) = std::env::var("XDG_DATA_HOME") {
        PathBuf::from(data).join("harmonia")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "freebsd")]
pub(super) fn lib_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("lib").join("harmonia")
    } else {
        PathBuf::from("/usr/local/lib/harmonia")
    }
}

#[cfg(target_os = "freebsd")]
pub(super) fn share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

#[cfg(target_os = "windows")]
pub(super) fn lib_dir() -> PathBuf {
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
pub(super) fn share_dir() -> PathBuf {
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
pub(super) fn lib_dir() -> PathBuf {
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
pub(super) fn share_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("share").join("harmonia")
    } else {
        PathBuf::from("/usr/local/share/harmonia")
    }
}

// --- Runtime and log dirs ---

#[cfg(target_os = "macos")]
pub(super) fn run_dir() -> PathBuf {
    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        PathBuf::from(tmpdir).join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia")
    }
}

#[cfg(target_os = "macos")]
pub(super) fn log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join("Library").join("Logs").join("Harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "linux")]
pub(super) fn run_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(xdg).join("harmonia")
    } else {
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/harmonia-{}", uid))
    }
}

#[cfg(target_os = "linux")]
pub(super) fn log_dir() -> PathBuf {
    if let Ok(state) = std::env::var("XDG_STATE_HOME") {
        PathBuf::from(state).join("harmonia")
    } else if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "freebsd")]
pub(super) fn run_dir() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/harmonia-{}", uid))
}

#[cfg(target_os = "freebsd")]
pub(super) fn log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}

#[cfg(target_os = "windows")]
pub(super) fn run_dir() -> PathBuf {
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
pub(super) fn log_dir() -> PathBuf {
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

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
pub(super) fn run_dir() -> PathBuf {
    PathBuf::from("/tmp/harmonia")
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "windows"
)))]
pub(super) fn log_dir() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".local").join("state").join("harmonia")
    } else {
        PathBuf::from("/tmp/harmonia/log")
    }
}
