use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProfile {
    FullAgent,
    TuiClient,
}

impl InstallProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FullAgent => "full-agent",
            Self::TuiClient => "tui-client",
        }
    }

    pub fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tui-client" | "client" | "tui" => Self::TuiClient,
            _ => Self::FullAgent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeRole {
    Agent,
    TuiClient,
    MqttClient,
}

impl NodeRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::TuiClient => "tui-client",
            Self::MqttClient => "mqtt-client",
        }
    }

    pub fn default_for_profile(profile: InstallProfile) -> Self {
        match profile {
            InstallProfile::FullAgent => Self::Agent,
            InstallProfile::TuiClient => Self::TuiClient,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub label: String,
    pub hostname: String,
    pub role: NodeRole,
    pub install_profile: InstallProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub id: String,
    pub node_label: String,
    pub node_role: NodeRole,
    pub install_profile: InstallProfile,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SessionPaths {
    pub identity: SessionIdentity,
    pub dir: PathBuf,
    pub manifest_path: PathBuf,
    pub events_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct SessionEvent<'a> {
    ts_ms: u64,
    actor: &'a str,
    kind: &'a str,
    text: &'a str,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn detect_hostname() -> String {
    std::env::var("HARMONIA_NODE_LABEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .or_else(|| std::env::var("HOST").ok())
        .or_else(|| std::env::var("COMPUTERNAME").ok())
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "harmonia-node".to_string())
}

fn sanitize_node_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_dash = false;
    for ch in raw.trim().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            prev_dash = false;
            ch.to_ascii_lowercase()
        } else if matches!(ch, '-' | '_' | '.') {
            prev_dash = false;
            ch
        } else if prev_dash {
            continue;
        } else {
            prev_dash = true;
            '-'
        };
        out.push(mapped);
    }
    let trimmed = out
        .trim_matches(|c: char| c == '-' || c == '_' || c == '.')
        .to_string();
    if trimmed.is_empty() {
        "harmonia-node".to_string()
    } else {
        trimmed
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(path, format!("{json}\n"))?;
    Ok(())
}

fn ensure_state_root_env() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", dir.to_string_lossy().as_ref());
    }
    Ok(dir)
}

pub fn config_value(scope: &str, key: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let _ = ensure_state_root_env()?;
    harmonia_config_store::init_v2()?;
    Ok(harmonia_config_store::get_config(
        "harmonia-cli",
        scope,
        key,
    )?)
}

pub fn set_config_value(
    scope: &str,
    key: &str,
    value: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = ensure_state_root_env()?;
    harmonia_config_store::init_v2()?;
    harmonia_config_store::set_config("harmonia-cli", scope, key, value)?;
    Ok(())
}

pub fn node_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(data_dir()?.join("config").join("node.json"))
}

pub fn nodes_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = data_dir()?.join("nodes");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(nodes_root()?.join(sanitize_node_label(label)))
}

pub fn node_sessions_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("sessions");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_pairings_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("pairings");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn node_memory_dir(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = node_dir(label)?.join("memory");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn node_manifest_path(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(node_dir(label)?.join("node.json"))
}

fn current_session_path(label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(node_dir(label)?.join("current-session"))
}

pub fn detect_install_profile() -> InstallProfile {
    std::env::var("HARMONIA_INSTALL_PROFILE")
        .map(|raw| InstallProfile::parse(&raw))
        .or_else(|_| {
            config_value("node", "install-profile")
                .ok()
                .flatten()
                .map(|raw| InstallProfile::parse(&raw))
                .ok_or(std::env::VarError::NotPresent)
        })
        .unwrap_or(InstallProfile::FullAgent)
}

pub fn ensure_node_layout(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(data_dir()?)?;
    fs::create_dir_all(data_dir()?.join("config"))?;
    fs::create_dir_all(node_dir(&identity.label)?)?;
    fs::create_dir_all(node_sessions_dir(&identity.label)?)?;
    fs::create_dir_all(node_pairings_dir(&identity.label)?)?;
    fs::create_dir_all(node_memory_dir(&identity.label)?)?;
    Ok(())
}

pub fn persist_node_identity(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    ensure_node_layout(identity)?;
    write_json(&node_config_path()?, identity)?;
    write_json(&node_manifest_path(&identity.label)?, identity)?;
    Ok(())
}

pub fn sync_runtime_config(identity: &NodeIdentity) -> Result<(), Box<dyn std::error::Error>> {
    let state_root = ensure_state_root_env()?;
    let lib_dir = lib_dir()?;
    let share_dir = share_dir()?;
    let run_dir = run_dir()?;
    let log_dir = log_dir()?;
    let wallet_root = wallet_root_path()?;
    let wallet_db = wallet_db_path()?;
    let vault_db = vault_db_path()?;

    let entries = [
        (
            "global",
            "state-root",
            state_root.to_string_lossy().to_string(),
        ),
        (
            "global",
            "system-dir",
            state_root.to_string_lossy().to_string(),
        ),
        (
            "global",
            "data-dir",
            state_root.to_string_lossy().to_string(),
        ),
        ("global", "lib-dir", lib_dir.to_string_lossy().to_string()),
        (
            "global",
            "share-dir",
            share_dir.to_string_lossy().to_string(),
        ),
        ("global", "run-dir", run_dir.to_string_lossy().to_string()),
        ("global", "log-dir", log_dir.to_string_lossy().to_string()),
        (
            "global",
            "wallet-root",
            wallet_root.to_string_lossy().to_string(),
        ),
        (
            "global",
            "wallet-db",
            wallet_db.to_string_lossy().to_string(),
        ),
        ("global", "vault-db", vault_db.to_string_lossy().to_string()),
        ("node", "label", identity.label.clone()),
        ("node", "hostname", identity.hostname.clone()),
        ("node", "role", identity.role.as_str().to_string()),
        (
            "node",
            "install-profile",
            identity.install_profile.as_str().to_string(),
        ),
        (
            "node",
            "sessions-root",
            node_sessions_dir(&identity.label)?
                .to_string_lossy()
                .to_string(),
        ),
        (
            "node",
            "pairings-root",
            node_pairings_dir(&identity.label)?
                .to_string_lossy()
                .to_string(),
        ),
        (
            "node",
            "memory-root",
            node_memory_dir(&identity.label)?
                .to_string_lossy()
                .to_string(),
        ),
    ];
    for (scope, key, value) in entries {
        set_config_value(scope, key, &value)?;
    }
    std::env::set_var(
        "HARMONIA_WALLET_ROOT",
        wallet_root.to_string_lossy().as_ref(),
    );
    std::env::set_var(
        "HARMONIA_VAULT_WALLET_DB",
        wallet_db.to_string_lossy().as_ref(),
    );
    Ok(())
}

pub fn load_node_identity() -> Result<Option<NodeIdentity>, Box<dyn std::error::Error>> {
    let path = node_config_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)?;
    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn current_node_identity() -> Result<NodeIdentity, Box<dyn std::error::Error>> {
    if let Some(identity) = load_node_identity()? {
        ensure_node_layout(&identity)?;
        sync_runtime_config(&identity)?;
        return Ok(identity);
    }

    let install_profile = detect_install_profile();
    let hostname = detect_hostname();
    let identity = NodeIdentity {
        label: sanitize_node_label(&hostname),
        hostname,
        role: NodeRole::default_for_profile(install_profile),
        install_profile,
    };
    persist_node_identity(&identity)?;
    sync_runtime_config(&identity)?;
    Ok(identity)
}

fn new_session_id() -> String {
    format!("session-{}", now_ms())
}

fn session_dir(label: &str, session_id: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(node_sessions_dir(label)?.join(session_id))
}

fn session_manifest_path(
    label: &str,
    session_id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(session_dir(label, session_id)?.join("session.json"))
}

fn session_events_path(
    label: &str,
    session_id: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(session_dir(label, session_id)?.join("events.jsonl"))
}

pub fn write_current_session(
    label: &str,
    session_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = current_session_path(label)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{session_id}\n"))?;
    Ok(())
}

fn load_current_session(label: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let path = current_session_path(label)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

pub fn resume_or_create_session(
    identity: &NodeIdentity,
) -> Result<SessionPaths, Box<dyn std::error::Error>> {
    ensure_node_layout(identity)?;
    let session_id = match load_current_session(&identity.label)? {
        Some(existing) if session_manifest_path(&identity.label, &existing)?.exists() => existing,
        _ => new_session_id(),
    };

    let dir = session_dir(&identity.label, &session_id)?;
    fs::create_dir_all(&dir)?;
    let manifest_path = session_manifest_path(&identity.label, &session_id)?;
    let events_path = session_events_path(&identity.label, &session_id)?;
    let now = now_ms();
    let identity_record = if manifest_path.exists() {
        let raw = fs::read_to_string(&manifest_path)?;
        let mut stored: SessionIdentity = serde_json::from_str(&raw)?;
        stored.updated_at_ms = now;
        stored
    } else {
        SessionIdentity {
            id: session_id.clone(),
            node_label: identity.label.clone(),
            node_role: identity.role,
            install_profile: identity.install_profile,
            created_at_ms: now,
            updated_at_ms: now,
        }
    };
    write_json(&manifest_path, &identity_record)?;
    write_current_session(&identity.label, &session_id)?;

    Ok(SessionPaths {
        identity: identity_record,
        dir,
        manifest_path,
        events_path,
    })
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub event_count: usize,
}

pub fn list_sessions(label: &str) -> Result<Vec<SessionSummary>, Box<dyn std::error::Error>> {
    let sessions_dir = node_sessions_dir(label)?;
    let mut summaries = Vec::new();

    let entries = match fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return Ok(summaries),
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let manifest = path.join("session.json");
        if !manifest.exists() {
            continue;
        }
        let raw = match fs::read_to_string(&manifest) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let identity: SessionIdentity = match serde_json::from_str(&raw) {
            Ok(i) => i,
            Err(_) => continue,
        };

        let events_path = path.join("events.jsonl");
        let event_count = if events_path.exists() {
            match fs::read_to_string(&events_path) {
                Ok(content) => content.lines().filter(|l| !l.trim().is_empty()).count(),
                Err(_) => 0,
            }
        } else {
            0
        };

        summaries.push(SessionSummary {
            id,
            created_at_ms: identity.created_at_ms,
            updated_at_ms: identity.updated_at_ms,
            event_count,
        });
    }

    summaries.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(summaries)
}

pub fn format_timestamp_ms(ms: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_millis(ms);
    // Format as local time: YYYY-MM-DD HH:MM
    // Use chrono-free approach: compute from SystemTime
    match dt.duration_since(UNIX_EPOCH) {
        Ok(dur) => {
            let secs = dur.as_secs() as i64;
            // Get local UTC offset via libc on unix, fallback to UTC
            #[cfg(unix)]
            let offset_secs = {
                let mut tm: libc::tm = unsafe { std::mem::zeroed() };
                unsafe { libc::localtime_r(&secs, &mut tm) };
                tm.tm_gmtoff
            };
            #[cfg(not(unix))]
            let offset_secs: i64 = 0;

            let local_secs = secs + offset_secs;
            // Days since epoch
            let days = local_secs.div_euclid(86400);
            let day_secs = local_secs.rem_euclid(86400);
            let hours = day_secs / 3600;
            let minutes = (day_secs % 3600) / 60;

            // Civil date from days since 1970-01-01 (algorithm from Howard Hinnant)
            let z = days + 719468;
            let era = z.div_euclid(146097);
            let doe = z.rem_euclid(146097);
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if m <= 2 { y + 1 } else { y };

            format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, hours, minutes)
        }
        Err(_) => "unknown".to_string(),
    }
}

pub fn append_session_event(
    session: &SessionPaths,
    actor: &str,
    kind: &str,
    text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = session.events_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let event = SessionEvent {
        ts_ms: now_ms(),
        actor,
        kind,
        text,
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&session.events_path)?;
    serde_json::to_writer(&mut file, &event)?;
    file.write_all(b"\n")?;

    let mut updated = session.identity.clone();
    updated.updated_at_ms = now_ms();
    write_json(&session.manifest_path, &updated)?;
    Ok(())
}

/// Read the user workspace path from config/workspace.sexp
pub fn user_workspace() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let config = data_dir()?.join("config").join("workspace.sexp");
    let content = std::fs::read_to_string(&config)?;
    if let Some(start) = content.find(":user-workspace") {
        let rest = &content[start..];
        if let Some(q1) = rest.find('"') {
            if let Some(q2) = rest[q1 + 1..].find('"') {
                return Ok(PathBuf::from(&rest[q1 + 1..q1 + 1 + q2]));
            }
        }
    }
    Err("workspace path not configured — run `harmonia setup`".into())
}

/// User data: ~/.harmoniis/harmonia/
/// Contains config.db, vault.db, config/, state/, frontends/ — nothing else.
pub fn data_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("HARMONIA_DATA_DIR") {
        if !path.trim().is_empty() {
            let dir = PathBuf::from(path);
            fs::create_dir_all(&dir)?;
            return Ok(dir);
        }
    }
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    Ok(home.join(".harmoniis").join("harmonia"))
}

/// Application libraries (cdylibs): ~/.local/lib/harmonia/
/// Platform-standard location for user-installed shared libraries.
pub fn lib_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_LIB_DIR") {
        if path.trim().is_empty() {
            platform_lib_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = config_value("global", "lib-dir") {
        PathBuf::from(path)
    } else {
        platform_lib_dir()
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Application data (source, docs, genesis): ~/.local/share/harmonia/
/// Platform-standard location for user-installed application data.
pub fn share_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = if let Ok(path) = std::env::var("HARMONIA_SHARE_DIR") {
        if path.trim().is_empty() {
            platform_share_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = config_value("global", "share-dir") {
        PathBuf::from(path)
    } else {
        platform_share_dir()
    };
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
    let dir = if let Ok(path) = std::env::var("HARMONIA_RUN_DIR") {
        if path.trim().is_empty() {
            platform_run_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = config_value("global", "run-dir") {
        PathBuf::from(path)
    } else {
        platform_run_dir()
    };
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
    let dir = if let Ok(path) = std::env::var("HARMONIA_LOG_DIR") {
        if path.trim().is_empty() {
            platform_log_dir()
        } else {
            PathBuf::from(path)
        }
    } else if let Ok(Some(path)) = config_value("global", "log-dir") {
        PathBuf::from(path)
    } else {
        platform_log_dir()
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

fn canonical_wallet_root(home: &Path) -> PathBuf {
    home.join(".harmoniis").join("wallet")
}

fn wallet_root_from_master_path(path: &Path) -> PathBuf {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "master.db" | "rgb.db" | "wallet.db" | "webcash.db" | "bitcoin.db"
            )
        })
        .unwrap_or(false)
    {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    }
}

fn configured_wallet_root() -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    if let Ok(path) = std::env::var("HARMONIA_WALLET_ROOT") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(PathBuf::from(trimmed)));
        }
    }
    if let Ok(path) = std::env::var("HARMONIA_VAULT_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(path) = std::env::var("HARMONIA_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(path) = std::env::var("HARMONIIS_WALLET_DB") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    if let Ok(Some(path)) = config_value("global", "wallet-root") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(PathBuf::from(trimmed)));
        }
    }
    if let Ok(Some(path)) = config_value("global", "wallet-db") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(Some(wallet_root_from_master_path(Path::new(trimmed))));
        }
    }
    Ok(None)
}

pub fn wallet_root_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;
    if let Some(root) = configured_wallet_root()? {
        return Ok(root);
    }
    Ok(canonical_wallet_root(&home))
}

/// The master wallet DB path. The wallet root also contains sibling DBs
/// managed by harmoniis-wallet: rgb.db, webcash.db, bitcoin.db.
pub fn wallet_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(wallet_root_path()?.join("master.db"))
}

pub fn vault_db_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(data_dir()?.join("vault.db"))
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
