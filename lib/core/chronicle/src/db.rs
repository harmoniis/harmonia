use rusqlite::Connection;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::schema;

/// Database connection type alias for passing through function signatures.
pub type DbConn = Mutex<Connection>;

/// Actor-owned chronicle state. The Connection is behind a Mutex for
/// thread-safe access within the actor (ractor may call from different tasks).
pub struct ChronicleState {
    pub conn: DbConn,
}

impl ChronicleState {
    /// Open a new chronicle database. Called once by the actor's pre_start.
    pub fn open() -> Result<Self, String> {
        let connection = open_connection()?;
        Ok(Self {
            conn: Mutex::new(connection),
        })
    }

    /// Get a lock on the database connection.
    pub fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, String> {
        self.conn.lock().map_err(|e| format!("chronicle lock poisoned: {e}"))
    }
}

fn state_root() -> PathBuf {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    let root = harmonia_config_store::get_config_or("chronicle", "global", "state-root", &default)
        .unwrap_or_else(|_| default);
    PathBuf::from(root)
}

fn db_path() -> PathBuf {
    if let Some(v) = harmonia_config_store::get_own("chronicle", "db")
        .ok()
        .flatten()
    {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    state_root().join("chronicle.db")
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
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-65536;
        PRAGMA busy_timeout=10000;
        PRAGMA foreign_keys=ON;
        PRAGMA temp_store=MEMORY;
        ",
    )
    .map_err(|e| e.to_string())?;

    schema::run_migrations(&conn)?;
    Ok(conn)
}

// ── Process-level state for callers outside the actor runtime ────────
// Phoenix and Ouroboros call chronicle directly (separate process or early init).
// Same pattern as vault: one process-level state, actor takes over for dispatch.

use std::sync::OnceLock;
static PROCESS_STATE: OnceLock<ChronicleState> = OnceLock::new();

pub(crate) fn conn() -> Result<&'static Mutex<Connection>, String> {
    if let Some(state) = PROCESS_STATE.get() {
        return Ok(&state.conn);
    }
    let state = ChronicleState::open()?;
    Ok(&PROCESS_STATE.get_or_init(|| state).conn)
}

pub fn init() -> Result<(), String> {
    let _ = conn()?;
    Ok(())
}

/// Run an arbitrary SQL query and return results as an s-expression string.
/// Supports SELECT only — mutations are rejected.
pub fn query_sexp(sql: &str) -> Result<String, String> {
    let trimmed = sql.trim();
    let upper = trimmed.to_ascii_uppercase();
    if !upper.starts_with("SELECT") && !upper.starts_with("WITH") {
        return Err("only SELECT / WITH queries allowed".to_string());
    }

    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock.prepare(trimmed).map_err(|e| e.to_string())?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let mut rows_sexp = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let mut fields = Vec::new();
        for (i, name) in col_names.iter().enumerate() {
            let val = row
                .get_ref(i)
                .map(|v| match v {
                    rusqlite::types::ValueRef::Null => "nil".to_string(),
                    rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                    rusqlite::types::ValueRef::Real(f) => format!("{:.6}", f),
                    rusqlite::types::ValueRef::Text(t) => {
                        let s = String::from_utf8_lossy(t);
                        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                    }
                    rusqlite::types::ValueRef::Blob(b) => format!("#<blob {}>", b.len()),
                })
                .unwrap_or_else(|_| "nil".to_string());
            fields.push(format!(":{} {}", name.replace('_', "-"), val));
        }
        rows_sexp.push(format!("({})", fields.join(" ")));
    }
    Ok(format!("({})", rows_sexp.join("\n ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_tables() {
        std::env::set_var("HARMONIA_CHRONICLE_DB", ":memory:");
        assert!(init().is_ok());
    }

    #[test]
    fn chronicle_state_opens() {
        std::env::set_var("HARMONIA_CHRONICLE_DB", ":memory:");
        let state = ChronicleState::open();
        assert!(state.is_ok());
    }
}
