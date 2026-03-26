use rusqlite::params;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::db;

pub fn record(
    event_type: &str,
    entries_created: i32,
    entries_source: i32,
    old_size: i64,
    new_size: i64,
    node_count: i32,
    edge_count: i32,
    interdisciplinary_edges: i32,
    max_depth: i32,
    detail: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO memory_events
            (event_type, entries_created, entries_source,
             old_size, new_size, compression_ratio,
             node_count, edge_count, interdisciplinary_edges,
             max_depth, detail)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            event_type,
            entries_created,
            entries_source,
            old_size,
            new_size,
            if old_size > 0 {
                Some(new_size as f64 / old_size as f64)
            } else {
                None
            },
            node_count,
            edge_count,
            interdisciplinary_edges,
            max_depth,
            detail,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
// Persistent memory entries — the agent's living memory
// ═══════════════════════════════════════════════════════════════════════

fn content_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Persist a memory entry. Dedup via content hash — same content never stored twice.
/// If hash matches existing entry, increment access_count instead of inserting.
pub fn persist_entry(
    id: &str,
    ts: i64,
    content: &str,
    tags: &str,
    source_ids: &str,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let hash = content_hash(content);

    // Check dedup — if content already exists, just update access
    let existing: Option<String> = lock
        .query_row(
            "SELECT id FROM memory_entries WHERE content_hash = ?1",
            params![hash],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_id) = existing {
        // Content exists — increment access, merge tags
        lock.execute(
            "UPDATE memory_entries SET access_count = access_count + 1, last_access = ?1, tags = tags || ' ' || ?2 WHERE id = ?3",
            params![ts, tags, existing_id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        // New content — insert
        lock.execute(
            "INSERT OR REPLACE INTO memory_entries (id, ts, content, tags, source_ids, access_count, content_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![id, ts, content, tags, source_ids, hash],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Load all memory entries from persistent store.
/// Returns (id, ts, content, tags, source_ids, access_count) tuples as sexp.
pub fn load_all_entries() -> Result<String, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock
        .prepare("SELECT id, ts, content, tags, source_ids, access_count FROM memory_entries ORDER BY ts ASC")
        .map_err(|e| e.to_string())?;

    let mut entries = Vec::new();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        if let Ok((id, ts, content, tags, source_ids, access_count)) = row {
            entries.push(format!(
                "(:id \"{}\" :ts {} :content \"{}\" :tags \"{}\" :source-ids \"{}\" :access-count {})",
                id.replace('"', "\\\""),
                ts,
                content.replace('"', "\\\"").replace('\n', "\\n"),
                tags,
                source_ids,
                access_count,
            ));
        }
    }

    Ok(format!("(:ok :count {} :entries ({}))", entries.len(), entries.join(" ")))
}

/// Update access count for a memory entry.
pub fn update_access(id: &str) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "UPDATE memory_entries SET access_count = access_count + 1, last_access = CAST(strftime('%s','now') AS INTEGER) WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Count total persistent memory entries.
pub fn entry_count() -> Result<i64, String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.query_row("SELECT COUNT(*) FROM memory_entries", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}
