use rusqlite::params;

use crate::db;

pub fn record(
    event_type: &str,
    exit_code: Option<i32>,
    attempt: Option<i32>,
    max_attempts: Option<i32>,
    recovery_ms: Option<i64>,
    detail: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO phoenix_events
            (event_type, exit_code, attempt, max_attempts, recovery_ms, detail)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![event_type, exit_code, attempt, max_attempts, recovery_ms, detail],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
