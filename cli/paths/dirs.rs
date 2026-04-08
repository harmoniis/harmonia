use std::path::PathBuf;

/// User data: ~/.harmoniis/harmonia/
/// Contains config.db, vault.db, config/, state/, frontends/ — nothing else.
pub fn data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("HARMONIA_DATA_DIR") {
        if !path.trim().is_empty() {
            let dir = PathBuf::from(path);
            std::fs::create_dir_all(&dir)?;
            return Ok(dir);
        }
    }
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    Ok(home.join(".harmoniis").join("harmonia"))
}

/// Application libraries (cdylibs): ~/.local/lib/harmonia/
pub fn lib_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_LIB_DIR") {
        if path.trim().is_empty() {
            super::platform::lib_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = super::config_value("global", "lib-dir") {
        PathBuf::from(path)
    } else {
        super::platform::lib_dir()
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Application data (source, docs, genesis): ~/.local/share/harmonia/
pub fn share_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_SHARE_DIR") {
        if path.trim().is_empty() {
            super::platform::share_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = super::config_value("global", "share-dir") {
        PathBuf::from(path)
    } else {
        super::platform::share_dir()
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Installed Lisp source tree: ~/.local/share/harmonia/src/
pub fn source_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(share_dir()?.join("src"))
}

/// Runtime directory for PID files and sockets.
pub fn run_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_RUN_DIR") {
        if path.trim().is_empty() {
            super::platform::run_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = super::config_value("global", "run-dir") {
        PathBuf::from(path)
    } else {
        super::platform::run_dir()
    };
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(dir)
}

/// Log directory.
pub fn log_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_LOG_DIR") {
        if path.trim().is_empty() {
            super::platform::log_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = super::config_value("global", "log-dir") {
        PathBuf::from(path)
    } else {
        super::platform::log_dir()
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn pid_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia.pid"))
}

pub fn broker_pid_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia-mqtt-broker.pid"))
}

pub fn node_service_pid_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(run_dir()?.join("harmonia-node-service.pid"))
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

pub fn node_service_log_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(log_dir()?.join("harmonia-node-service.log"))
}
