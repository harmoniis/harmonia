//! Database operations: path resolution, connection opening, schema init.

use std::env;
use std::path::PathBuf;

use rusqlite::Connection;

pub(crate) fn bool_env(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

pub(super) fn state_root() -> PathBuf {
    env::var("HARMONIA_STATE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir().join("harmonia"))
}

pub fn store_path() -> PathBuf {
    env::var("HARMONIA_VAULT_DB")
        .or_else(|_| env::var("HARMONIA_VAULT_PATH"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_root().join("vault.db"))
}

pub(super) fn open_db() -> Result<Connection, String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("vault db dir create failed: {e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("vault db open failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS secrets (
            symbol TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("vault schema init failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS vault_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            op TEXT NOT NULL,
            symbol TEXT NOT NULL,
            source TEXT DEFAULT ''
        )",
        [],
    )
    .map_err(|e| format!("vault audit schema init failed: {e}"))?;
    Ok(conn)
}
