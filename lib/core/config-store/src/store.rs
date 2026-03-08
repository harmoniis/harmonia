use rusqlite::{params, Connection};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

fn connect() -> Result<Connection, String> {
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
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn norm_scope(scope: &str) -> String {
    let v = scope.trim().to_ascii_lowercase();
    if v.is_empty() {
        "global".to_string()
    } else {
        v
    }
}

fn norm_key(key: &str) -> String {
    key.trim().to_ascii_lowercase()
}

pub fn init() -> Result<(), String> {
    let _ = connect()?;
    Ok(())
}

pub fn set_value(scope: &str, key: &str, value: &str) -> Result<(), String> {
    let scope = norm_scope(scope);
    let key = norm_key(key);
    if key.is_empty() {
        return Err("empty config key".to_string());
    }

    let conn = connect()?;
    conn.execute(
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
    Ok(())
}

pub fn get_value(scope: &str, key: &str) -> Result<Option<String>, String> {
    let scope = norm_scope(scope);
    let key = norm_key(key);
    if key.is_empty() {
        return Ok(None);
    }

    let conn = connect()?;
    let mut stmt = conn
        .prepare("SELECT value FROM config_kv WHERE scope=?1 AND key=?2")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![scope, key]).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => row.get::<_, String>(0).map(Some).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}

pub fn list_keys(scope: Option<&str>) -> Result<Vec<String>, String> {
    let conn = connect()?;
    match scope {
        Some(v) => {
            let scoped = norm_scope(v);
            let mut stmt = conn
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
            let mut stmt = conn
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
