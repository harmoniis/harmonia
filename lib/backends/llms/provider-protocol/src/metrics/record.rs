//! Write functions: recording LLM perf, parallel tasks, and tmux events.

use rusqlite::params;

use super::db::{db, debug_logging_enabled, now_secs};

/// Record an LLM model invocation.
pub fn record_llm_perf(
    backend: &str,
    model: &str,
    latency_ms: u128,
    success: bool,
    usd_in_1k: f64,
    usd_out_1k: f64,
) {
    let ts = now_secs();
    if debug_logging_enabled() {
        eprintln!(
            "[DEBUG] [{}] perf: model={} latency={}ms ok={}",
            backend, model, latency_ms, success
        );
    }
    if let Ok(conn) = db().lock() {
        let _ = conn.execute(
            "INSERT INTO llm_perf (ts, backend, model, latency_ms, success, usd_in_1k, usd_out_1k)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                ts,
                backend,
                model,
                latency_ms as i64,
                success as i32,
                usd_in_1k,
                usd_out_1k
            ],
        );
    }
}

/// Record a parallel-agent task completion.
pub fn record_parallel_task(
    task_id: u64,
    model: &str,
    latency_ms: u64,
    cost_usd: f64,
    success: bool,
    verified: bool,
    verification_source: &str,
    verification_detail: &str,
) {
    let ts = now_secs();
    if let Ok(conn) = db().lock() {
        let _ = conn.execute(
            "INSERT INTO parallel_tasks (ts, task_id, model, latency_ms, cost_usd, success, verified, verification_source, verification_detail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                ts, task_id as i64, model, latency_ms as i64, cost_usd,
                success as i32, verified as i32, verification_source, verification_detail
            ],
        );
    }
}

/// Record a tmux agent event.
pub fn record_tmux_event(
    agent_id: u64,
    cli_type: &str,
    session_name: &str,
    workdir: &str,
    event: &str,
    interaction_count: u64,
    inputs_sent: u64,
    cost_usd: f64,
    duration_ms: u64,
) {
    let ts = now_secs();
    if let Ok(conn) = db().lock() {
        let _ = conn.execute(
            "INSERT INTO tmux_events (ts, agent_id, cli_type, session_name, workdir, event, interaction_count, inputs_sent, cost_usd, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                ts, agent_id as i64, cli_type, session_name, workdir, event,
                interaction_count as i64, inputs_sent as i64, cost_usd, duration_ms as i64
            ],
        );
    }
}
