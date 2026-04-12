//! MemPalace event recording and persistence.

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

// ═══════════════════════════════════════════════════════════════════════
// Palace persistence — drawers, graph nodes, graph edges
// ═══════════════════════════════════════════════════════════════════════

#[deprecated(note = "Palace data now persists to .md files, not Chronicle")]
/// Persist a single drawer (verbatim content).
pub fn persist_drawer(
    id: u64,
    content: &str,
    source: &str,
    room_id: u32,
    chunk_index: u16,
    created_at: u64,
    tags: &str,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT OR REPLACE INTO palace_drawers (id, content, source, room_id, chunk_index, created_at, tags)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id as i64, content, source, room_id as i64, chunk_index as i64, created_at as i64, tags],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[deprecated(note = "Palace data now persists to .md files, not Chronicle")]
/// Persist a graph node.
pub fn persist_node(
    id: u32,
    kind: &str,
    label: &str,
    domain: &str,
    created_at: u64,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT OR REPLACE INTO palace_nodes (id, kind, label, domain, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id as i64, kind, label, domain, created_at as i64],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[deprecated(note = "Palace data now persists to .md files, not Chronicle")]
/// Persist a graph edge.
pub fn persist_edge(
    source_id: u32,
    target_id: u32,
    kind: &str,
    weight: f64,
    confidence: f64,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT OR IGNORE INTO palace_edges (source_id, target_id, kind, weight, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![source_id as i64, target_id as i64, kind, weight, confidence],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Load all drawers from Chronicle for warm-start.
pub fn load_drawers() -> Result<Vec<(u64, String, String, u32, u16, u64, String)>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare(
            "SELECT id, content, source, room_id, chunk_index, created_at, tags
             FROM palace_drawers ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? as u32,
                row.get::<_, i64>(4)? as u16,
                row.get::<_, i64>(5)? as u64,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Load all graph nodes from Chronicle.
pub fn load_nodes() -> Result<Vec<(u32, String, String, String, u64)>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare(
            "SELECT id, kind, label, domain, created_at FROM palace_nodes ORDER BY id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)? as u64,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

/// Load all graph edges from Chronicle.
pub fn load_edges() -> Result<Vec<(u32, u32, String, f64, f64)>, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare(
            "SELECT source_id, target_id, kind, weight, confidence FROM palace_edges",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u32,
                row.get::<_, i64>(1)? as u32,
                row.get::<_, String>(2)?,
                row.get::<_, f64>(3)?,
                row.get::<_, f64>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
