use rusqlite::params;

use crate::db;

pub fn record(
    task_hint: Option<&str>,
    model_chosen: &str,
    backend: &str,
    reason: Option<&str>,
    escalated: bool,
    escalated_from: Option<&str>,
    cost_usd: f64,
    latency_ms: i64,
    success: bool,
    tokens_in: i64,
    tokens_out: i64,
) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO delegation_log
            (task_hint, model_chosen, backend, reason,
             escalated, escalated_from,
             cost_usd, latency_ms, success, tokens_in, tokens_out)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            task_hint,
            model_chosen,
            backend,
            reason,
            escalated as i32,
            escalated_from,
            cost_usd,
            latency_ms,
            success as i32,
            tokens_in,
            tokens_out,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
