//! Raw SQL query execution and pre-built query functions for agent decisions.

use rusqlite::params;

use super::db::{db, is_readonly_sql, sexp_escape};

/// Execute an arbitrary SELECT query and return results as JSON array of objects.
/// Only SELECT/WITH/EXPLAIN statements are allowed (read-only).
/// Returns JSON string: `[{"col1": val1, "col2": val2}, ...]`
pub fn query_sql(sql: &str) -> Result<String, String> {
    if !is_readonly_sql(sql) {
        return Err("only SELECT/WITH/EXPLAIN/PRAGMA queries allowed".to_string());
    }

    let conn = db()
        .lock()
        .map_err(|_| "metrics db lock poisoned".to_string())?;

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("sql prepare error: {e}"))?;

    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let mut fields = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => "null".to_string(),
                    Ok(rusqlite::types::ValueRef::Integer(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Real(v)) => format!("{v}"),
                    Ok(rusqlite::types::ValueRef::Text(v)) => {
                        let s = String::from_utf8_lossy(v);
                        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                    }
                    Ok(rusqlite::types::ValueRef::Blob(_)) => "\"<blob>\"".to_string(),
                    Err(_) => "null".to_string(),
                };
                fields.push(format!("\"{}\":{}", col_names[i], val));
            }
            Ok(format!("{{{}}}", fields.join(",")))
        })
        .map_err(|e| format!("sql query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(format!("[{}]", rows.join(",")))
}

/// Execute a SELECT query and return results as s-expression for Lisp.
/// Each row becomes a plist: `(:col1 val1 :col2 val2 ...)`
/// Returns: `((row1) (row2) ...)`
pub fn query_sql_sexp(sql: &str) -> Result<String, String> {
    if !is_readonly_sql(sql) {
        return Err("only SELECT/WITH/EXPLAIN/PRAGMA queries allowed".to_string());
    }

    let conn = db()
        .lock()
        .map_err(|_| "metrics db lock poisoned".to_string())?;

    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("sql prepare error: {e}"))?;

    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| {
            let name = stmt.column_name(i).unwrap_or("col");
            // Convert snake_case to kebab-case for Lisp
            format!(":{}", name.replace('_', "-"))
        })
        .collect();

    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let mut fields = Vec::with_capacity(col_count);
            for i in 0..col_count {
                let val = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => "nil".to_string(),
                    Ok(rusqlite::types::ValueRef::Integer(v)) => v.to_string(),
                    Ok(rusqlite::types::ValueRef::Real(v)) => format!("{v}"),
                    Ok(rusqlite::types::ValueRef::Text(v)) => {
                        let s = String::from_utf8_lossy(v);
                        format!("\"{}\"", sexp_escape(&s))
                    }
                    Ok(rusqlite::types::ValueRef::Blob(_)) => "nil".to_string(),
                    Err(_) => "nil".to_string(),
                };
                fields.push(format!("{} {}", col_names[i], val));
            }
            Ok(format!("({})", fields.join(" ")))
        })
        .map_err(|e| format!("sql query error: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(format!("({})", rows.join(" ")))
}

/// Model performance summary as s-expression.
pub fn query_model_stats(model: &str) -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "(:error \"metrics lock\")".to_string(),
    };
    let result: Result<(i64, i64, f64), _> = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(success),0), COALESCE(AVG(latency_ms),0) FROM llm_perf WHERE model = ?1",
        params![model],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    match result {
        Ok((count, successes, avg_lat)) if count > 0 => {
            let sr = successes as f64 / count as f64;
            let (usd_in, usd_out) = conn
                .query_row(
                    "SELECT usd_in_1k, usd_out_1k FROM llm_perf WHERE model = ?1 ORDER BY ts DESC LIMIT 1",
                    params![model],
                    |row| Ok((row.get::<_, f64>(0)?, row.get::<_, f64>(1)?)),
                )
                .unwrap_or((0.0, 0.0));
            format!(
                "(:model \"{}\" :count {} :success-rate {:.4} :avg-latency-ms {:.1} :usd-in-1k {:.6} :usd-out-1k {:.6})",
                model, count, sr, avg_lat, usd_in, usd_out
            )
        }
        _ => format!("(:model \"{}\" :count 0)", model),
    }
}

/// Best-performing models for a backend.
pub fn query_best_models_for_task(backend: &str, limit: i32) -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "()".to_string(),
    };
    let mut stmt = match conn.prepare(
        "SELECT model, COUNT(*) as cnt, AVG(CAST(success AS REAL)) as sr, AVG(latency_ms) as lat
         FROM llm_perf WHERE backend = ?1
         GROUP BY model HAVING cnt >= 2
         ORDER BY sr DESC, lat ASC LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return "()".to_string(),
    };
    let rows: Vec<String> = stmt
        .query_map(params![backend, limit], |row| {
            let model: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let sr: f64 = row.get(2)?;
            let lat: f64 = row.get(3)?;
            Ok(format!(
                "(:model \"{}\" :count {} :success-rate {:.4} :avg-latency-ms {:.1})",
                model, cnt, sr, lat
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    format!("({})", rows.join(" "))
}

/// Full parallel-agent performance report.
pub fn query_performance_report() -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "(:error \"metrics lock\")".to_string(),
    };

    let (total, successes, total_cost, avg_lat): (i64, i64, f64, f64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(success),0), COALESCE(SUM(cost_usd),0), COALESCE(AVG(latency_ms),0)
             FROM parallel_tasks",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap_or((0, 0, 0.0, 0.0));

    let verified: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(verified),0) FROM parallel_tasks",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let sr = if total > 0 {
        successes as f64 / total as f64
    } else {
        0.0
    };
    let vr = if total > 0 {
        verified as f64 / total as f64
    } else {
        0.0
    };

    let mut stmt = match conn.prepare(
        "SELECT model, COUNT(*), SUM(success), SUM(verified), SUM(cost_usd), AVG(latency_ms)
         FROM parallel_tasks GROUP BY model ORDER BY model",
    ) {
        Ok(s) => s,
        Err(_) => {
            return format!(
                "(:total {} :success-rate {:.4} :verified-rate {:.4} :total-cost-usd {:.8} :avg-latency-ms {:.2} :models ())",
                total, sr, vr, total_cost, avg_lat
            );
        }
    };

    let model_bits: Vec<String> = stmt
        .query_map([], |row| {
            let model: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let ok: i64 = row.get(2)?;
            let ver: i64 = row.get(3)?;
            let cost: f64 = row.get(4)?;
            let lat: f64 = row.get(5)?;
            let msr = if cnt > 0 { ok as f64 / cnt as f64 } else { 0.0 };
            let mvr = if cnt > 0 { ver as f64 / cnt as f64 } else { 0.0 };
            Ok(format!(
                "(:model \"{}\" :count {} :success-rate {:.4} :verified-rate {:.4} :cost-usd {:.8} :avg-latency-ms {:.2})",
                model, cnt, msr, mvr, cost, lat
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    format!(
        "(:total {} :success-rate {:.4} :verified-rate {:.4} :total-cost-usd {:.8} :avg-latency-ms {:.2} :models ({}))",
        total, sr, vr, total_cost, avg_lat, model_bits.join(" ")
    )
}

/// LLM backend performance report.
pub fn query_llm_report() -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "(:error \"metrics lock\")".to_string(),
    };

    let mut stmt = match conn.prepare(
        "SELECT backend, model, COUNT(*), SUM(success), AVG(latency_ms), usd_in_1k, usd_out_1k
         FROM llm_perf GROUP BY backend, model ORDER BY backend, model",
    ) {
        Ok(s) => s,
        Err(_) => return "()".to_string(),
    };

    let entries: Vec<String> = stmt
        .query_map([], |row| {
            let backend: String = row.get(0)?;
            let model: String = row.get(1)?;
            let cnt: i64 = row.get(2)?;
            let ok: i64 = row.get(3)?;
            let lat: f64 = row.get(4)?;
            let usd_in: f64 = row.get(5)?;
            let usd_out: f64 = row.get(6)?;
            let sr = if cnt > 0 { ok as f64 / cnt as f64 } else { 0.0 };
            Ok(format!(
                "(:backend \"{}\" :model \"{}\" :count {} :success-rate {:.4} :avg-latency-ms {:.1} :usd-in-1k {:.6} :usd-out-1k {:.6})",
                backend, model, cnt, sr, lat, usd_in, usd_out
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    format!("({})", entries.join(" "))
}

/// Tmux agent event summary.
pub fn query_tmux_report() -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "(:error \"metrics lock\")".to_string(),
    };

    let mut stmt = match conn.prepare(
        "SELECT agent_id, cli_type, session_name, COUNT(*) as events,
                MAX(interaction_count) as interactions, MAX(inputs_sent) as inputs,
                SUM(cost_usd) as total_cost, SUM(duration_ms) as total_duration
         FROM tmux_events GROUP BY agent_id ORDER BY agent_id DESC LIMIT 50",
    ) {
        Ok(s) => s,
        Err(_) => return "()".to_string(),
    };

    let entries: Vec<String> = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let cli: String = row.get(1)?;
            let sess: String = row.get(2)?;
            let evts: i64 = row.get(3)?;
            let ints: i64 = row.get(4)?;
            let inp: i64 = row.get(5)?;
            let cost: f64 = row.get(6)?;
            let dur: i64 = row.get(7)?;
            Ok(format!(
                "(:id {} :cli-type \"{}\" :session \"{}\" :events {} :interactions {} :inputs {} :cost-usd {:.6} :duration-ms {})",
                id, cli, sess, evts, ints, inp, cost, dur
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    format!("({})", entries.join(" "))
}

/// Combined telemetry digest.
pub fn query_telemetry_digest() -> String {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return "(:error \"metrics lock\")".to_string(),
    };

    let mut stmt = match conn.prepare(
        "SELECT model, COUNT(*) as cnt, AVG(CAST(success AS REAL)) as sr,
                AVG(latency_ms) as lat, usd_in_1k, usd_out_1k
         FROM llm_perf GROUP BY model ORDER BY cnt DESC LIMIT 10",
    ) {
        Ok(s) => s,
        Err(_) => return "(:llm () :tmux () :catalogue 0)".to_string(),
    };

    let llm_entries: Vec<String> = stmt
        .query_map([], |row| {
            let model: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let sr: f64 = row.get(2)?;
            let lat: f64 = row.get(3)?;
            let usd_in: f64 = row.get(4)?;
            let usd_out: f64 = row.get(5)?;
            Ok(format!(
                "(:model \"{}\" :n {} :sr {:.3} :lat {:.0} :$/ki {:.5} :$/ko {:.5})",
                model, cnt, sr, lat, usd_in, usd_out
            ))
        })
        .ok()
        .map(|iter| iter.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    let tmux_summary: (i64, i64, f64) = conn
        .query_row(
            "SELECT COUNT(DISTINCT agent_id), COUNT(*), COALESCE(SUM(cost_usd), 0.0) FROM tmux_events",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap_or((0, 0, 0.0));

    let catalogue_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM models", [], |row| row.get(0))
        .unwrap_or(0);

    format!(
        "(:llm ({}) :tmux (:agents {} :events {} :cost-usd {:.6}) :catalogue {})",
        llm_entries.join(" "),
        tmux_summary.0,
        tmux_summary.1,
        tmux_summary.2,
        catalogue_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_query_safety() {
        // Should reject non-SELECT
        assert!(query_sql("DROP TABLE models").is_err());
        assert!(query_sql("DELETE FROM llm_perf").is_err());
        assert!(query_sql("INSERT INTO models VALUES(1)").is_err());
        assert!(query_sql("UPDATE models SET name='x'").is_err());
    }
}
