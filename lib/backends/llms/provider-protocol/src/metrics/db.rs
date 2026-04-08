//! Database initialization, connection management, and shared helpers.

use rusqlite::Connection;
use std::env;
use std::sync::{Mutex, OnceLock};

const COMPONENT: &str = "provider-protocol";

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

pub(crate) fn db() -> &'static Mutex<Connection> {
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

pub(super) fn init_tables(conn: &Connection) {
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

pub(super) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Escape a string for s-expression output.
/// CL's reader handles literal newlines in strings natively -- do NOT escape them.
pub(super) fn sexp_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(super) fn is_readonly_sql(sql: &str) -> bool {
    let trimmed = sql.trim_start().to_ascii_uppercase();
    trimmed.starts_with("SELECT")
        || trimmed.starts_with("WITH")
        || trimmed.starts_with("EXPLAIN")
        || trimmed.starts_with("PRAGMA TABLE_INFO")
        || trimmed.starts_with("PRAGMA INDEX_LIST")
}

pub(super) fn debug_logging_enabled() -> bool {
    harmonia_config_store::get_config_or(COMPONENT, "global", "log-level", "info")
        .map(|v| v == "debug")
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

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
}
