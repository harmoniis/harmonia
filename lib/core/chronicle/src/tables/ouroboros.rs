use rusqlite::params;

use crate::db;

pub fn record(
    event_type: &str,
    component: Option<&str>,
    detail: Option<&str>,
    patch_size: Option<i64>,
    success: bool,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO ouroboros_events
            (event_type, component, detail, patch_size, success)
         VALUES (?1,?2,?3,?4,?5)",
        params![event_type, component, detail, patch_size, success as i32],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
