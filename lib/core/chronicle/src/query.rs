use crate::db;

/// Recent harmonic history as s-expression plist.
pub fn harmonic_history(since_ts: i64, limit: i32) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, cycle, phase, strength, utility, beauty, signal, noise,
                chaos_risk, lorenz_x, lorenz_y, lorenz_z, lorenz_bounded,
                lambdoma_ratio, lambdoma_convergent, rewrite_ready, rewrite_count,
                security_posture, security_events
         FROM harmonic_snapshots
         WHERE ts >= {}
         ORDER BY ts DESC
         LIMIT {}",
        since_ts, limit
    );
    db::query_sexp(&sql)
}

/// Harmony trajectory (downsampled 5-min buckets).
pub fn harmony_trajectory(since_ts: i64, until_ts: i64) -> Result<String, String> {
    let until_clause = if until_ts > 0 {
        format!("AND bucket_ts <= {}", until_ts)
    } else {
        String::new()
    };
    let sql = format!(
        "SELECT bucket_ts, sample_count, avg_signal, min_signal, max_signal,
                avg_chaos_risk, avg_strength, avg_utility, avg_beauty
         FROM harmony_trajectory
         WHERE bucket_ts >= {} {}
         ORDER BY bucket_ts",
        since_ts, until_clause
    );
    db::query_sexp(&sql)
}

/// Memory event history.
pub fn memory_history(since_ts: i64, limit: i32) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, event_type, entries_created, entries_source,
                old_size, new_size, compression_ratio,
                node_count, edge_count, interdisciplinary_edges,
                max_depth, detail
         FROM memory_events
         WHERE ts >= {}
         ORDER BY ts DESC
         LIMIT {}",
        since_ts, limit
    );
    db::query_sexp(&sql)
}

/// Phoenix event history.
pub fn phoenix_history(since_ts: i64, limit: i32) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, event_type, exit_code, attempt, max_attempts, recovery_ms, detail
         FROM phoenix_events
         WHERE ts >= {}
         ORDER BY ts DESC
         LIMIT {}",
        since_ts, limit
    );
    db::query_sexp(&sql)
}

/// Ouroboros event history.
pub fn ouroboros_history(since_ts: i64, limit: i32) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, event_type, component, detail, patch_size, success
         FROM ouroboros_events
         WHERE ts >= {}
         ORDER BY ts DESC
         LIMIT {}",
        since_ts, limit
    );
    db::query_sexp(&sql)
}

/// Delegation history.
pub fn delegation_history(since_ts: i64, limit: i32) -> Result<String, String> {
    let sql = format!(
        "SELECT ts, task_hint, model_chosen, backend, reason,
                escalated, escalated_from, cost_usd, latency_ms,
                success, tokens_in, tokens_out
         FROM delegation_log
         WHERE ts >= {}
         ORDER BY ts DESC
         LIMIT {}",
        since_ts, limit
    );
    db::query_sexp(&sql)
}

/// Summary: current harmony state from latest snapshot.
pub fn harmony_summary() -> Result<String, String> {
    db::query_sexp(
        "SELECT ts, cycle, phase, strength, utility, beauty, signal, noise,
                chaos_risk, lorenz_bounded, lambdoma_ratio, lambdoma_convergent,
                rewrite_ready, rewrite_count, security_posture, security_events
         FROM harmonic_snapshots
         ORDER BY ts DESC
         LIMIT 1",
    )
}

/// Delegation report: model performance aggregates.
pub fn delegation_report() -> Result<String, String> {
    db::query_sexp(
        "SELECT model_chosen,
                COUNT(*) AS uses,
                SUM(cost_usd) AS total_cost,
                ROUND(AVG(latency_ms)) AS avg_latency,
                ROUND(100.0 * SUM(success) / COUNT(*), 1) AS success_pct,
                SUM(escalated) AS escalations,
                SUM(tokens_in) AS total_tokens_in,
                SUM(tokens_out) AS total_tokens_out
         FROM delegation_log
         GROUP BY model_chosen
         ORDER BY uses DESC",
    )
}

/// Cost report: spending over time windows.
pub fn cost_report(since_ts: i64) -> Result<String, String> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let day = 24 * 3600 * 1000_i64;

    let sql = format!(
        "SELECT
            (SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= {h24}) AS cost_24h,
            (SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= {h7d}) AS cost_7d,
            (SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= {h30d}) AS cost_30d,
            (SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= {since}) AS cost_since,
            (SELECT model_chosen FROM delegation_log WHERE ts >= {since}
                GROUP BY model_chosen ORDER BY SUM(cost_usd) DESC LIMIT 1) AS most_expensive_model,
            (SELECT model_chosen FROM delegation_log WHERE ts >= {since}
                GROUP BY model_chosen ORDER BY SUM(cost_usd) ASC LIMIT 1) AS cheapest_model,
            (SELECT COUNT(DISTINCT model_chosen) FROM delegation_log WHERE ts >= {since}) AS unique_models",
        h24 = now_ms - day,
        h7d = now_ms - 7 * day,
        h30d = now_ms - 30 * day,
        since = since_ts,
    );

    db::query_sexp(&sql)
}

/// Full digest: compact overview of all subsystems.
pub fn full_digest() -> Result<String, String> {
    let harmony = harmony_summary().unwrap_or_else(|_| "()".to_string());
    let delegation = delegation_report().unwrap_or_else(|_| "()".to_string());
    let cost = cost_report(0).unwrap_or_else(|_| "()".to_string());

    let recent_memory = db::query_sexp(
        "SELECT event_type, COUNT(*) AS n
         FROM memory_events
         WHERE ts >= (CAST(strftime('%s','now') AS INTEGER) * 1000 - 86400000)
         GROUP BY event_type"
    ).unwrap_or_else(|_| "()".to_string());

    let recent_phoenix = db::query_sexp(
        "SELECT event_type, COUNT(*) AS n
         FROM phoenix_events
         WHERE ts >= (CAST(strftime('%s','now') AS INTEGER) * 1000 - 86400000)
         GROUP BY event_type"
    ).unwrap_or_else(|_| "()".to_string());

    let recent_ouroboros = db::query_sexp(
        "SELECT event_type, COUNT(*) AS n
         FROM ouroboros_events
         WHERE ts >= (CAST(strftime('%s','now') AS INTEGER) * 1000 - 86400000)
         GROUP BY event_type"
    ).unwrap_or_else(|_| "()".to_string());

    let graph_stats = db::query_sexp(
        "SELECT node_count, edge_count, interdisciplinary_edges
         FROM graph_snapshots
         ORDER BY ts DESC LIMIT 1"
    ).unwrap_or_else(|_| "()".to_string());

    Ok(format!(
        "(:harmony {} :delegation {} :cost {} :memory-24h {} :phoenix-24h {} :ouroboros-24h {} :graph {})",
        harmony, delegation, cost, recent_memory, recent_phoenix, recent_ouroboros, graph_stats
    ))
}
