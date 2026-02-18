use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use rusqlite::{params, Connection};

fn state_root() -> PathBuf {
    env::var("HARMONIA_STATE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir().join("harmonia"))
}

pub fn store_path() -> PathBuf {
    env::var("HARMONIA_VAULT_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_root().join("vault.db"))
}

pub fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().trim_start_matches(':').to_ascii_lowercase()
}

pub fn normalize_env_symbol(raw: &str) -> String {
    normalize_symbol(&raw.to_ascii_lowercase().replace("__", "-"))
}

fn open_db() -> Result<Connection, String> {
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
    Ok(conn)
}

pub fn load_store_file() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let conn = match open_db() {
        Ok(v) => v,
        Err(_) => return map,
    };
    let mut stmt = match conn.prepare("SELECT symbol, value FROM secrets") {
        Ok(v) => v,
        Err(_) => return map,
    };
    let rows = match stmt.query_map([], |row| {
        let symbol: String = row.get(0)?;
        let value: String = row.get(1)?;
        Ok((symbol, value))
    }) {
        Ok(v) => v,
        Err(_) => return map,
    };
    for (symbol, value) in rows.flatten() {
        map.insert(normalize_symbol(&symbol), value);
    }
    map
}

pub fn upsert_secret(symbol: &str, value: &str) -> Result<(), String> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO secrets(symbol, value) VALUES (?1, ?2)
         ON CONFLICT(symbol) DO UPDATE SET value=excluded.value",
        params![normalize_symbol(symbol), value],
    )
    .map_err(|e| format!("vault upsert failed: {e}"))?;
    Ok(())
}

pub fn list_symbols() -> Result<Vec<String>, String> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT symbol FROM secrets ORDER BY symbol ASC")
        .map_err(|e| format!("vault list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("vault list query failed: {e}"))?;
    let mut out = Vec::new();
    for symbol in rows.flatten() {
        out.push(normalize_symbol(&symbol));
    }
    Ok(out)
}

pub fn has_symbol(symbol: &str) -> Result<bool, String> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT 1 FROM secrets WHERE symbol = ?1 LIMIT 1")
        .map_err(|e| format!("vault has prepare failed: {e}"))?;
    let mut rows = stmt
        .query(params![normalize_symbol(symbol)])
        .map_err(|e| format!("vault has query failed: {e}"))?;
    match rows.next() {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => Err(format!("vault has row read failed: {e}")),
    }
}

pub fn load_legacy_kv_into_db_if_present() -> Result<(), String> {
    let legacy_path = env::var("HARMONIA_VAULT_STORE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_root().join("vault.secrets"));
    if !legacy_path.exists() {
        return Ok(());
    }
    let body = std::fs::read_to_string(&legacy_path)
        .map_err(|e| format!("vault legacy read failed: {e}"))?;
    for line in body.lines() {
        if let Some((k, v)) = line.split_once('=') {
            upsert_secret(k, v)?;
        }
    }
    Ok(())
}
