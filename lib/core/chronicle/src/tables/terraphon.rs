//! Terraphon datamining event recording.

use crate::db;
use rusqlite::params;

/// Record a terraphon datamining event.
pub fn record(
    event_type: &str,
    lode_id: &str,
    platform: &str,
    node_label: &str,
    domain: &str,
    strategy: &str,
    elapsed_ms: i64,
    result_size: i64,
    compressed: bool,
    cross_node: bool,
    origin_node: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO terraphon_events
            (event_type, lode_id, platform, node_label, domain, strategy,
             elapsed_ms, result_size, compressed, cross_node, origin_node, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            event_type,
            lode_id,
            platform,
            node_label,
            domain,
            strategy,
            elapsed_ms,
            result_size,
            compressed as i32,
            cross_node as i32,
            origin_node,
            error,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Query recent terraphon events.
pub fn recent(limit: usize) -> Result<Vec<String>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare(
            "SELECT event_type, lode_id, platform, elapsed_ms, result_size, error
             FROM terraphon_events ORDER BY ts DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit as i64], |row| {
            let event_type: String = row.get(0)?;
            let lode_id: String = row.get(1)?;
            let platform: String = row.get(2)?;
            let elapsed_ms: i64 = row.get(3)?;
            let result_size: i64 = row.get(4)?;
            let error: Option<String> = row.get(5)?;
            Ok(format!(
                "(:type \"{}\" :lode \"{}\" :platform \"{}\" :elapsed-ms {} :size {} :error {})",
                event_type, lode_id, platform, elapsed_ms, result_size,
                error.map_or("nil".to_string(), |e| format!("\"{}\"", e)),
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}
