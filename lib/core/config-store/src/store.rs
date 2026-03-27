use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, RwLock};

// ─── Singleton connection ───────────────────────────────────────────

/// The config DB connection type. Public so ConfigStoreActor can own one directly.
pub type ConfigDbConn = Mutex<Connection>;

static DB_CONN: OnceLock<ConfigDbConn> = OnceLock::new();

fn state_root() -> PathBuf {
    if let Ok(v) = env::var("HARMONIA_STATE_ROOT") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    env::temp_dir().join("harmonia")
}

fn db_path() -> PathBuf {
    if let Ok(v) = env::var("HARMONIA_CONFIG_DB") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    state_root().join("config.db")
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_connection() -> Result<Connection, String> {
    let path = db_path();
    ensure_parent(&path)?;
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        CREATE TABLE IF NOT EXISTS config_kv (
            scope TEXT NOT NULL,
            key TEXT NOT NULL,
            value TEXT NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            PRIMARY KEY(scope, key)
        );
        CREATE INDEX IF NOT EXISTS idx_config_updated_at ON config_kv(updated_at);
        CREATE TABLE IF NOT EXISTS config_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn conn() -> Result<&'static ConfigDbConn, String> {
    if let Some(c) = DB_CONN.get() {
        return Ok(c);
    }
    let connection = open_connection()?;
    Ok(DB_CONN.get_or_init(|| Mutex::new(connection)))
}

// ─── In-memory cache ────────────────────────────────────────────────

/// The config cache type. Public so ConfigStoreActor can own one directly.
pub type ConfigCache = RwLock<HashMap<(String, String), String>>;

static CACHE: OnceLock<ConfigCache> = OnceLock::new();

fn cache() -> &'static ConfigCache {
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn cache_get(scope: &str, key: &str) -> Option<String> {
    cache()
        .read()
        .ok()
        .and_then(|m| m.get(&(scope.to_string(), key.to_string())).cloned())
}

pub(crate) fn cache_set(scope: &str, key: &str, value: &str) {
    if let Ok(mut m) = cache().write() {
        m.insert((scope.to_string(), key.to_string()), value.to_string());
    }
}

pub(crate) fn cache_remove(scope: &str, key: &str) {
    if let Ok(mut m) = cache().write() {
        m.remove(&(scope.to_string(), key.to_string()));
    }
}

// ─── Normalization ──────────────────────────────────────────────────

pub(crate) fn norm_scope(scope: &str) -> String {
    let v = scope.trim().to_ascii_lowercase();
    if v.is_empty() {
        "global".to_string()
    } else {
        v
    }
}

pub(crate) fn norm_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

// ─── Public API ─────────────────────────────────────────────────────

pub fn init() -> Result<(), String> {
    let _ = conn()?;
    // Load all DB values into cache
    for (scope, key, value) in load_all()? {
        cache_set(&scope, &key, &value);
    }
    Ok(())
}

pub fn set_value(scope: &str, key: &str, value: &str) -> Result<(), String> {
    let scope = norm_scope(scope);
    let key = norm_key(key);
    if key.is_empty() {
        return Err("empty config key".to_string());
    }

    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "
        INSERT INTO config_kv(scope, key, value, updated_at)
        VALUES (?1, ?2, ?3, strftime('%s','now'))
        ON CONFLICT(scope, key) DO UPDATE SET
            value=excluded.value,
            updated_at=excluded.updated_at
        ",
        params![scope, key, value],
    )
    .map_err(|e| e.to_string())?;
    cache_set(&scope, &key, value);
    Ok(())
}

pub fn get_value(scope: &str, key: &str) -> Result<Option<String>, String> {
    let scope = norm_scope(scope);
    let key = norm_key(key);
    if key.is_empty() {
        return Ok(None);
    }

    // Cache first
    if let Some(v) = cache_get(&scope, &key) {
        return Ok(Some(v));
    }

    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare("SELECT value FROM config_kv WHERE scope=?1 AND key=?2")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![scope, key]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => {
            let v: String = row.get(0).map_err(|e| e.to_string())?;
            cache_set(&scope, &key, &v);
            Ok(Some(v))
        }
        None => Ok(None),
    }
}

pub fn delete_value(scope: &str, key: &str) -> Result<(), String> {
    let scope = norm_scope(scope);
    let key = norm_key(key);
    if key.is_empty() {
        return Err("empty config key".to_string());
    }

    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "DELETE FROM config_kv WHERE scope=?1 AND key=?2",
        params![scope, key],
    )
    .map_err(|e| e.to_string())?;
    cache_remove(&scope, &key);
    Ok(())
}

pub fn list_keys(scope: Option<&str>) -> Result<Vec<String>, String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    match scope {
        Some(v) => {
            let scoped = norm_scope(v);
            let mut stmt = lock
                .prepare("SELECT key FROM config_kv WHERE scope=?1 ORDER BY key")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map(params![scoped], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row.map_err(|e| e.to_string())?);
            }
            Ok(out)
        }
        None => {
            let mut stmt = lock
                .prepare("SELECT scope, key FROM config_kv ORDER BY scope, key")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| e.to_string())?;
            let mut out = Vec::new();
            for row in rows {
                let (scope, key) = row.map_err(|e| e.to_string())?;
                out.push(format!("{scope}:{key}"));
            }
            Ok(out)
        }
    }
}

pub fn dump_scope(scope: &str) -> Result<Vec<(String, String)>, String> {
    let scope = norm_scope(scope);
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare("SELECT key, value FROM config_kv WHERE scope=?1 ORDER BY key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![scope], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

pub fn load_all() -> Result<Vec<(String, String, String)>, String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare("SELECT scope, key, value FROM config_kv ORDER BY scope, key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// ─── config_meta helpers ────────────────────────────────────────────

pub fn get_meta(key: &str) -> Result<Option<String>, String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare("SELECT value FROM config_meta WHERE key=?1")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![key]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => row.get::<_, String>(0).map(Some).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

pub fn set_meta(key: &str, value: &str) -> Result<(), String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT OR REPLACE INTO config_meta(key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
