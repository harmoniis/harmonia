//! MemPalace event recording.

use crate::db;
use rusqlite::params;

/// Record a palace event (node/edge/drawer operations).
pub fn record(
    event_type: &str,
    operation: &str,
    node_id: Option<i64>,
    label: Option<&str>,
    detail: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO palace_events
            (event_type, operation, node_id, label, detail)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![event_type, operation, node_id, label, detail],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Query recent palace events.
pub fn recent(limit: usize) -> Result<Vec<String>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare(
            "SELECT event_type, operation, node_id, label, detail
             FROM palace_events ORDER BY ts DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            let event_type: String = row.get(0)?;
            let operation: String = row.get(1)?;
            let node_id: Option<i64> = row.get(2)?;
            let label: Option<String> = row.get(3)?;
            let detail: Option<String> = row.get(4)?;
            Ok(format!(
                "(:type \"{}\" :op \"{}\" :node {} :label {} :detail {})",
                event_type,
                operation,
                node_id.map_or("nil".to_string(), |n| n.to_string()),
                label.map_or("nil".to_string(), |l| format!("\"{}\"", l)),
                detail.map_or("nil".to_string(), |d| format!("\"{}\"", d)),
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}
