use rusqlite::params;

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
