//! SQLite-backed metrics store for model performance, catalogue, and tmux agent tracking.
//!
//! Single unified database: `{HARMONIA_STATE_ROOT}/metrics.db`
//! Tables:
//!   - `models`         — full model catalogue (synced from OpenRouter API + hardcoded)
//!   - `llm_perf`       — every LLM backend call with latency/success/pricing
//!   - `parallel_tasks` — parallel-agent task completions
//!   - `tmux_events`    — tmux CLI agent lifecycle events
//!
//! The agent can run arbitrary SELECT queries via `query_sql()` to get any data it needs.

use rusqlite::{params, Connection};
use std::env;
use std::sync::{Mutex, OnceLock};

const COMPONENT: &str = "provider-protocol";

// ---------------------------------------------------------------------------
// Database path + connection
// ---------------------------------------------------------------------------

fn state_root() -> String {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}

fn metrics_db_path() -> String {
    harmonia_config_store::get_config(COMPONENT, "global", "metrics-db")
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("{}/metrics.db", state_root()))
}

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

fn db() -> &'static Mutex<Connection> {
    DB.get_or_init(|| {
        let path = metrics_db_path();
        if let Some(parent) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path).expect("failed to open metrics.db");
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;\
             PRAGMA synchronous=NORMAL;\
             PRAGMA cache_size=-8000;\
             PRAGMA temp_store=MEMORY;",
        )
        .expect("failed to set pragmas");
        init_tables(&conn);
        Mutex::new(conn)
    })
}

fn init_tables(conn: &Connection) {
    conn.execute_batch(
        "
        -- Full model catalogue: synced from OpenRouter API + hardcoded backends
        CREATE TABLE IF NOT EXISTS models (
            id             TEXT PRIMARY KEY,
            name           TEXT NOT NULL DEFAULT '',
            provider       TEXT NOT NULL DEFAULT '',
            context_length INTEGER NOT NULL DEFAULT 0,
            max_completion INTEGER NOT NULL DEFAULT 0,
            usd_per_tok_in  REAL NOT NULL DEFAULT 0,
            usd_per_tok_out REAL NOT NULL DEFAULT 0,
            modality       TEXT NOT NULL DEFAULT 'text->text',
            source         TEXT NOT NULL DEFAULT 'hardcoded',
            updated_at     INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_models_provider ON models(provider);
        CREATE INDEX IF NOT EXISTS idx_models_price_in ON models(usd_per_tok_in);

        -- Per-invocation LLM performance
        CREATE TABLE IF NOT EXISTS llm_perf (
            ts         INTEGER NOT NULL,
            backend    TEXT    NOT NULL,
            model      TEXT    NOT NULL,
            latency_ms INTEGER NOT NULL,
            success    INTEGER NOT NULL,
            usd_in_1k  REAL NOT NULL,
            usd_out_1k REAL NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_llm_perf_model ON llm_perf(model);
        CREATE INDEX IF NOT EXISTS idx_llm_perf_backend ON llm_perf(backend);
        CREATE INDEX IF NOT EXISTS idx_llm_perf_ts ON llm_perf(ts);

        -- Parallel-agent task completions
        CREATE TABLE IF NOT EXISTS parallel_tasks (
            ts         INTEGER NOT NULL,
            task_id    INTEGER NOT NULL,
            model      TEXT    NOT NULL,
            latency_ms INTEGER NOT NULL,
            cost_usd   REAL    NOT NULL,
            success    INTEGER NOT NULL,
            verified   INTEGER NOT NULL,
            verification_source TEXT NOT NULL DEFAULT 'none',
            verification_detail TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_parallel_model ON parallel_tasks(model);

        -- Tmux CLI agent events
        CREATE TABLE IF NOT EXISTS tmux_events (
            ts               INTEGER NOT NULL,
            agent_id         INTEGER NOT NULL,
            cli_type         TEXT    NOT NULL,
            session_name     TEXT    NOT NULL,
            workdir          TEXT    NOT NULL,
            event            TEXT    NOT NULL,
            interaction_count INTEGER NOT NULL,
            inputs_sent      INTEGER NOT NULL,
            cost_usd         REAL    NOT NULL DEFAULT 0.0,
            duration_ms      INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_tmux_agent ON tmux_events(agent_id);
        CREATE INDEX IF NOT EXISTS idx_tmux_event ON tmux_events(event);
        ",
    )
    .expect("failed to create metrics tables");

    // Migration guard for existing databases that lack the new columns
    let _ = conn.execute_batch(
        "ALTER TABLE tmux_events ADD COLUMN cost_usd REAL NOT NULL DEFAULT 0.0;
         ALTER TABLE tmux_events ADD COLUMN duration_ms INTEGER NOT NULL DEFAULT 0;",
    );
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Escape a string for s-expression output.
fn sexp_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ---------------------------------------------------------------------------
// Write: LLM perf, parallel tasks, tmux events
// ---------------------------------------------------------------------------

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
    eprintln!(
        "[harmonia-{backend}] perf: ts={ts} model={model} latency={latency_ms}ms ok={success}"
    );
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

// ---------------------------------------------------------------------------
// Model catalogue: sync from OpenRouter API
// ---------------------------------------------------------------------------

/// Sync the model catalogue from the OpenRouter /api/v1/models endpoint.
/// Fetches all models with pricing and upserts into the `models` table.
/// Returns count of models synced, or error.
pub fn sync_models_from_openrouter(api_key: &str) -> Result<usize, String> {
    let output = std::process::Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg("10")
        .arg("--max-time")
        .arg("30")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("https://openrouter.ai/api/v1/models")
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("curl failed: {}", &stderr[..stderr.len().min(200)]));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("json parse failed: {e}"))?;

    let data = parsed
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "missing data array in response".to_string())?;

    let conn = db()
        .lock()
        .map_err(|_| "metrics db lock poisoned".to_string())?;

    let ts = now_secs();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("transaction failed: {e}"))?;

    let mut count = 0usize;
    for model in data {
        let id = match model.get("id").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let name = model.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let ctx = model
            .get("context_length")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let max_comp = model
            .get("max_completion_tokens")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let pricing = model.get("pricing");
        let usd_in = pricing
            .and_then(|p| p.get("prompt"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let usd_out = pricing
            .and_then(|p| p.get("completion"))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let modality = model
            .get("architecture")
            .and_then(|a| a.get("modality"))
            .and_then(|v| v.as_str())
            .unwrap_or("text->text");

        // Extract provider from id (e.g. "openai/gpt-5" -> "openai")
        let provider = id.split_once('/').map(|(p, _)| p).unwrap_or("");

        let _ = tx.execute(
            "INSERT INTO models (id, name, provider, context_length, max_completion, usd_per_tok_in, usd_per_tok_out, modality, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'openrouter-api', ?9)
             ON CONFLICT(id) DO UPDATE SET
               name=excluded.name, context_length=excluded.context_length,
               max_completion=excluded.max_completion, usd_per_tok_in=excluded.usd_per_tok_in,
               usd_per_tok_out=excluded.usd_per_tok_out, modality=excluded.modality,
               source='openrouter-api', updated_at=excluded.updated_at",
            params![id, name, provider, ctx, max_comp, usd_in, usd_out, modality, ts],
        );
        count += 1;
    }

    tx.commit().map_err(|e| format!("commit failed: {e}"))?;
    eprintln!("[harmonia-metrics] synced {count} models from OpenRouter API");
    Ok(count)
}

/// Insert hardcoded model offerings into the catalogue (lower priority than API data).
/// Only inserts if model not already present from API sync.
pub fn upsert_hardcoded_offerings(offerings: &[crate::ModelOffering], backend: &str) {
    let conn = match db().lock() {
        Ok(c) => c,
        Err(_) => return,
    };
    let ts = now_secs();
    for o in offerings {
        // Convert per-1k-token price to per-token price
        let usd_in = o.usd_in_1k / 1000.0;
        let usd_out = o.usd_out_1k / 1000.0;
        let provider = o.id.split_once('/').map(|(p, _)| p).unwrap_or(backend);
        // Only insert if not already present (don't overwrite API data)
        let _ = conn.execute(
            "INSERT OR IGNORE INTO models (id, name, provider, usd_per_tok_in, usd_per_tok_out, source, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'hardcoded', ?6)",
            params![o.id, o.id, provider, usd_in, usd_out, ts],
        );
    }
}

// ---------------------------------------------------------------------------
// Raw SQL query — the agent can ask anything
// ---------------------------------------------------------------------------

/// Execute an arbitrary SELECT query and return results as JSON array of objects.
/// Only SELECT/WITH/EXPLAIN statements are allowed (read-only).
/// Returns JSON string: `[{"col1": val1, "col2": val2}, ...]`
pub fn query_sql(sql: &str) -> Result<String, String> {
    // Safety: only allow read-only statements
    let trimmed = sql.trim_start().to_ascii_uppercase();
    if !trimmed.starts_with("SELECT")
        && !trimmed.starts_with("WITH")
        && !trimmed.starts_with("EXPLAIN")
        && !trimmed.starts_with("PRAGMA TABLE_INFO")
        && !trimmed.starts_with("PRAGMA INDEX_LIST")
    {
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
    let trimmed = sql.trim_start().to_ascii_uppercase();
    if !trimmed.starts_with("SELECT")
        && !trimmed.starts_with("WITH")
        && !trimmed.starts_with("EXPLAIN")
        && !trimmed.starts_with("PRAGMA TABLE_INFO")
        && !trimmed.starts_with("PRAGMA INDEX_LIST")
    {
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

// ---------------------------------------------------------------------------
// Pre-built query functions — curated data for common agent decisions
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Metrics → Harmonic Matrix Bridge
// ---------------------------------------------------------------------------

/// Query recent LLM performance data and return s-expression entries suitable
/// for the Lisp conductor to feed into `harmonic_matrix_observe_route()`.
///
/// Each entry: `(:route "backend/model" :latency-ms N :success-rate F :cost F)`
///
/// The Lisp conductor calls this, iterates results, and calls observe_route
/// for each — avoiding circular Rust crate dependencies.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_create_and_query() {
        let conn = Connection::open_in_memory().unwrap();
        init_tables(&conn);

        conn.execute(
            "INSERT INTO llm_perf VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![1000, "openrouter", "test/model", 150, 1, 0.001, 0.002],
        )
        .unwrap();

        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM llm_perf", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);

        // Verify models table exists
        conn.execute(
            "INSERT INTO models (id, name, provider, usd_per_tok_in, usd_per_tok_out, updated_at)
             VALUES ('test/m1', 'Test Model', 'test', 0.001, 0.002, 1000)",
            [],
        )
        .unwrap();

        let model_cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM models", [], |r| r.get(0))
            .unwrap();
        assert_eq!(model_cnt, 1);
    }

    #[test]
    fn sql_query_safety() {
        // Should reject non-SELECT
        assert!(query_sql("DROP TABLE models").is_err());
        assert!(query_sql("DELETE FROM llm_perf").is_err());
        assert!(query_sql("INSERT INTO models VALUES(1)").is_err());
        assert!(query_sql("UPDATE models SET name='x'").is_err());
    }
}
