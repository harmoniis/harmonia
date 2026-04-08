/// Session service — creates, stores, lists, resumes sessions per node.
/// Every frontend accesses sessions through IPC dispatch.
/// Storage: one JSON event per line in `{data_dir}/nodes/{label}/sessions/{id}/events.jsonl`

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Public types ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub node_label: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub events_dir: PathBuf,
    pub events_path: PathBuf,
    pub manifest_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub ts_ms: u64,
    pub actor: String,
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionManifest {
    id: String,
    node_label: String,
    created_at_ms: u64,
    updated_at_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn sessions_dir(node_label: &str, data_dir: &Path) -> PathBuf {
    data_dir.join("nodes").join(node_label).join("sessions")
}

fn session_dir(node_label: &str, data_dir: &Path, session_id: &str) -> PathBuf {
    sessions_dir(node_label, data_dir).join(session_id)
}

fn current_session_path(node_label: &str, data_dir: &Path) -> PathBuf {
    data_dir.join("nodes").join(node_label).join("current-session")
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| format!("json: {e}"))?;
    fs::write(path, format!("{json}\n")).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

fn build_session(node_label: &str, data_dir: &Path, manifest: &SessionManifest) -> Session {
    let dir = session_dir(node_label, data_dir, &manifest.id);
    Session {
        id: manifest.id.clone(),
        node_label: node_label.to_string(),
        created_at_ms: manifest.created_at_ms,
        updated_at_ms: manifest.updated_at_ms,
        events_dir: dir.clone(),
        events_path: dir.join("events.jsonl"),
        manifest_path: dir.join("session.json"),
    }
}

/// Create a new session for a node.
pub fn create(node_label: &str, data_dir: &Path) -> Result<Session, String> {
    let session_id = format!("session-{}", now_ms());
    init_session(node_label, data_dir, &session_id)
}

/// Resume a specific session by ID.
pub fn resume(node_label: &str, data_dir: &Path, session_id: &str) -> Result<Session, String> {
    init_session(node_label, data_dir, session_id)
}

/// List sessions for a node, newest first.
pub fn list(node_label: &str, data_dir: &Path) -> Result<Vec<SessionSummary>, String> {
    let dir = sessions_dir(node_label, data_dir);
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(Vec::new()),
    };

    let mut summaries = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let manifest_path = path.join("session.json");
        let manifest: SessionManifest = match fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
        {
            Some(m) => m,
            None => continue,
        };
        let events_path = path.join("events.jsonl");
        let event_count = fs::read_to_string(&events_path)
            .map(|c| c.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0);

        summaries.push(SessionSummary {
            id,
            created_at_ms: manifest.created_at_ms,
            updated_at_ms: manifest.updated_at_ms,
            event_count,
        });
    }
    summaries.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
    Ok(summaries)
}

/// Append an event to a session's log.
pub fn append_event(session: &Session, actor: &str, kind: &str, text: &str) -> Result<(), String> {
    if let Some(parent) = session.events_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    let event = SessionEvent {
        ts_ms: now_ms(),
        actor: actor.to_string(),
        kind: kind.to_string(),
        text: text.to_string(),
    };
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&session.events_path)
        .map_err(|e| format!("open: {e}"))?;
    serde_json::to_writer(&mut file, &event).map_err(|e| format!("write event: {e}"))?;
    file.write_all(b"\n").map_err(|e| format!("newline: {e}"))?;

    // Update manifest timestamp
    let manifest = SessionManifest {
        id: session.id.clone(),
        node_label: session.node_label.clone(),
        created_at_ms: session.created_at_ms,
        updated_at_ms: now_ms(),
    };
    write_json(&session.manifest_path, &manifest)
}

/// Read session events (for replay/rewind).
pub fn read_events(session: &Session) -> Result<Vec<SessionEvent>, String> {
    let content = fs::read_to_string(&session.events_path).unwrap_or_default();
    let events: Vec<SessionEvent> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Ok(events)
}

/// Get current active session for a node, if one is recorded.
pub fn current(node_label: &str, data_dir: &Path) -> Result<Option<Session>, String> {
    let path = current_session_path(node_label, data_dir);
    let session_id = match fs::read_to_string(&path) {
        Ok(raw) => {
            let trimmed = raw.trim().to_string();
            if trimmed.is_empty() {
                return Ok(None);
            }
            trimmed
        }
        Err(_) => return Ok(None),
    };
    let manifest_path = session_dir(node_label, data_dir, &session_id).join("session.json");
    match fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<SessionManifest>(&raw).ok())
    {
        Some(manifest) => Ok(Some(build_session(node_label, data_dir, &manifest))),
        None => Ok(None),
    }
}

/// Format a millisecond timestamp as a local-time string.
pub fn format_timestamp_ms(ms: u64) -> String {
    use std::time::Duration;
    let dt = UNIX_EPOCH + Duration::from_millis(ms);
    match dt.duration_since(UNIX_EPOCH) {
        Ok(dur) => {
            let secs = dur.as_secs() as i64;
            #[cfg(unix)]
            let offset_secs = {
                let mut tm: libc::tm = unsafe { std::mem::zeroed() };
                unsafe { libc::localtime_r(&secs, &mut tm) };
                tm.tm_gmtoff
            };
            #[cfg(not(unix))]
            let offset_secs: i64 = 0;

            let local_secs = secs + offset_secs;
            let days = local_secs.div_euclid(86400);
            let day_secs = local_secs.rem_euclid(86400);
            let hours = day_secs / 3600;
            let minutes = (day_secs % 3600) / 60;

            // Civil date from days since 1970-01-01 (Howard Hinnant algorithm)
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

fn init_session(node_label: &str, data_dir: &Path, session_id: &str) -> Result<Session, String> {
    let dir = session_dir(node_label, data_dir, session_id);
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir session: {e}"))?;
    let manifest_path = dir.join("session.json");
    let now = now_ms();

    let manifest = if manifest_path.exists() {
        let raw = fs::read_to_string(&manifest_path).map_err(|e| format!("read manifest: {e}"))?;
        let mut stored: SessionManifest =
            serde_json::from_str(&raw).map_err(|e| format!("parse manifest: {e}"))?;
        stored.updated_at_ms = now;
        stored
    } else {
        SessionManifest {
            id: session_id.to_string(),
            node_label: node_label.to_string(),
            created_at_ms: now,
            updated_at_ms: now,
        }
    };
    write_json(&manifest_path, &manifest)?;

    // Write current-session pointer
    let cs_path = current_session_path(node_label, data_dir);
    if let Some(parent) = cs_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&cs_path, format!("{session_id}\n")).map_err(|e| format!("write current: {e}"))?;

    Ok(build_session(node_label, data_dir, &manifest))
}

/// Resolve the data_dir from config-store (gateway reads state-root from config).
pub fn resolve_data_dir() -> Result<PathBuf, String> {
    harmonia_config_store::get_config("gateway", "global", "data-dir")
        .map_err(|e| format!("config-store: {e}"))?
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "data-dir not set in config-store".to_string())
}

/// Resolve the current node label from config-store.
pub fn resolve_node_label() -> Result<String, String> {
    harmonia_config_store::get_config("gateway", "node", "label")
        .map_err(|e| format!("config-store: {e}"))?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "node label not set in config-store".to_string())
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir() -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("harmonia_session_test_{}_{}", now_ms(), n));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn create_and_read_session() {
        let data_dir = temp_dir();
        let session = create("test-node", &data_dir).unwrap();
        assert!(session.id.starts_with("session-"));
        assert_eq!(session.node_label, "test-node");
        assert!(session.events_dir.exists());

        let events = read_events(&session).unwrap();
        assert!(events.is_empty());
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn append_and_read_events() {
        let data_dir = temp_dir();
        let session = create("test-node", &data_dir).unwrap();

        append_event(&session, "you", "user", "hello").unwrap();
        append_event(&session, "harmonia", "assistant", "hi there").unwrap();

        let events = read_events(&session).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].actor, "you");
        assert_eq!(events[0].kind, "user");
        assert_eq!(events[0].text, "hello");
        assert_eq!(events[1].actor, "harmonia");
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn list_sessions_sorted() {
        let data_dir = temp_dir();
        let _s1 = create("test-node", &data_dir).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _s2 = create("test-node", &data_dir).unwrap();

        let summaries = list("test-node", &data_dir).unwrap();
        assert_eq!(summaries.len(), 2);
        assert!(summaries[0].updated_at_ms >= summaries[1].updated_at_ms);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn current_session_tracking() {
        let data_dir = temp_dir();
        let session = create("test-node", &data_dir).unwrap();

        let cur = current("test-node", &data_dir).unwrap();
        assert!(cur.is_some());
        assert_eq!(cur.unwrap().id, session.id);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn resume_existing_session() {
        let data_dir = temp_dir();
        let session = create("test-node", &data_dir).unwrap();
        append_event(&session, "you", "user", "hello").unwrap();

        let resumed = resume("test-node", &data_dir, &session.id).unwrap();
        assert_eq!(resumed.id, session.id);
        let events = read_events(&resumed).unwrap();
        assert_eq!(events.len(), 1);
        let _ = fs::remove_dir_all(&data_dir);
    }

    #[test]
    fn format_timestamp() {
        let ts = format_timestamp_ms(1712500000000);
        assert!(ts.contains("2024"), "should be a 2024 date: {}", ts);
    }
}
