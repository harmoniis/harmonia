use rusqlite::params;

use crate::db;

pub fn record(
    event_type: &str,
    cycle: i64,
    confidence: f64,
    stability: f64,
    novelty: f64,
    reward: f64,
    accepted: bool,
    recall_hits: i32,
    checkpoint_path: Option<&str>,
    checkpoint_digest: Option<&str>,
    detail: Option<&str>,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO signalograd_events
            (event_type, cycle, confidence, stability, novelty, reward,
             accepted, recall_hits, checkpoint_path, checkpoint_digest, detail)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            event_type,
            cycle,
            confidence,
            stability,
            novelty,
            reward,
            accepted as i32,
            recall_hits,
            checkpoint_path,
            checkpoint_digest,
            detail,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
