//! Error event recording — structured, queryable error history.
//! The harmonic machine queries this to detect failure patterns.

use crate::db;
use rusqlite::params;

/// Record a structured error event.
pub fn record(
    source: &str,
    kind: &str,
    model: Option<&str>,
    detail: Option<&str>,
    latency_ms: i64,
    cascaded_to: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO error_events (source, kind, model, detail, latency_ms, cascaded_to)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![source, kind, model.unwrap_or(""), detail.unwrap_or(""), latency_ms, cascaded_to.unwrap_or("")],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Query recent errors, optionally filtered by source or kind.
pub fn recent(limit: usize, source_filter: Option<&str>, kind_filter: Option<&str>) -> Result<Vec<String>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT source, kind, model, detail, latency_ms, cascaded_to, ts FROM error_events {} ORDER BY ts DESC LIMIT ?",
        match (source_filter, kind_filter) {
            (Some(_), Some(_)) => "WHERE source = ? AND kind = ?",
            (Some(_), None) => "WHERE source = ?",
            (None, Some(_)) => "WHERE kind = ?",
            (None, None) => "",
        }
    );
    let mut stmt = lock.prepare(&sql).map_err(|e| e.to_string())?;
    let rows = match (source_filter, kind_filter) {
        (Some(s), Some(k)) => stmt.query_map(params![s, k, limit as i64], format_row),
        (Some(s), None) => stmt.query_map(params![s, limit as i64], format_row),
        (None, Some(k)) => stmt.query_map(params![k, limit as i64], format_row),
        (None, None) => stmt.query_map(params![limit as i64], format_row),
    }.map_err(|e| e.to_string())?;
    rows.filter_map(|r| r.ok()).collect::<Vec<_>>().into_iter().map(Ok).collect()
}

fn format_row(row: &rusqlite::Row) -> rusqlite::Result<String> {
    let source: String = row.get(0)?;
    let kind: String = row.get(1)?;
    let model: String = row.get(2)?;
    let detail: String = row.get(3)?;
    let latency: i64 = row.get(4)?;
    let cascaded: String = row.get(5)?;
    Ok(format!("(:source \"{}\" :kind \"{}\" :model \"{}\" :detail \"{}\" :latency-ms {} :cascaded-to \"{}\")",
        source, kind, model, detail, latency, cascaded))
}
