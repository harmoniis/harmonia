//! Metrics to Harmonic Matrix bridge: query recent LLM performance data
//! and return s-expression entries for the Lisp conductor.

use rusqlite::params;

use super::db::db;

/// Query recent LLM performance data and return s-expression entries suitable
/// for the Lisp conductor to feed into `harmonic_matrix_observe_route()`.
///
/// Each entry: `(:route "backend/model" :latency-ms N :success-rate F :cost F)`
///
/// The Lisp conductor calls this, iterates results, and calls observe_route
/// for each -- avoiding circular Rust crate dependencies.
pub fn bridge_perf_to_routes(since_ts: i64) -> Result<String, String> {
    let conn = db()
        .lock()
        .map_err(|_| "metrics db lock poisoned".to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT backend || '/' || model AS route,
                    AVG(latency_ms) AS lat,
                    AVG(CAST(success AS REAL)) AS sr,
                    AVG(usd_in_1k + usd_out_1k) AS cost,
                    COUNT(*) AS n
             FROM llm_perf WHERE ts > ?1
             GROUP BY backend, model ORDER BY n DESC",
        )
        .map_err(|e| format!("bridge query error: {e}"))?;

    let entries: Vec<String> = stmt
        .query_map(params![since_ts], |row| {
            let route: String = row.get(0)?;
            let lat: f64 = row.get(1)?;
            let sr: f64 = row.get(2)?;
            let cost: f64 = row.get(3)?;
            let n: i64 = row.get(4)?;
            Ok(format!(
                "(:route \"{}\" :latency-ms {:.0} :success-rate {:.4} :cost {:.6} :count {})",
                route, lat, sr, cost, n
            ))
        })
        .map_err(|e| format!("bridge query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(format!("({})", entries.join(" ")))
}
