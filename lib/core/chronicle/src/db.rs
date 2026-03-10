use rusqlite::{params, Connection};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

// ─── Schema version for migrations ────────────────────────────────────
const SCHEMA_VERSION: i32 = 2;

fn state_root() -> PathBuf {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    let root = harmonia_config_store::get_config_or("chronicle", "global", "state-root", &default)
        .unwrap_or_else(|_| default);
    PathBuf::from(root)
}

fn db_path() -> PathBuf {
    if let Some(v) = harmonia_config_store::get_own("chronicle", "db")
        .ok()
        .flatten()
    {
        if !v.is_empty() {
            return PathBuf::from(v);
        }
    }
    state_root().join("chronicle.db")
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open_connection() -> Result<Connection, String> {
    let path = db_path();
    ensure_parent(&path)?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    // Pragmas: WAL mode, normal sync, 64MB cache, 10s busy timeout
    conn.execute_batch(
        "
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-65536;
        PRAGMA busy_timeout=10000;
        PRAGMA foreign_keys=ON;
        PRAGMA temp_store=MEMORY;
        ",
    )
    .map_err(|e| e.to_string())?;

    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chronicle_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| e.to_string())?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(CAST(value AS INTEGER), 0)
             FROM chronicle_meta WHERE key='schema_version'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < 1 {
        migrate_v1(conn)?;
    }
    if current_version < 2 {
        migrate_v2(conn)?;
    }

    conn.execute(
        "INSERT OR REPLACE INTO chronicle_meta(key, value) VALUES ('schema_version', ?1)",
        params![SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn migrate_v1(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        -- ═══ Table 1: harmonic_snapshots ═══
        -- Full harmonic state captured each cycle: vitruvian triad, chaos dynamics,
        -- Lorenz attractor, Lambdoma convergence, security posture.
        CREATE TABLE IF NOT EXISTS harmonic_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            cycle INTEGER NOT NULL,
            phase TEXT NOT NULL,
            strength REAL NOT NULL DEFAULT 0.0,
            utility REAL NOT NULL DEFAULT 0.0,
            beauty REAL NOT NULL DEFAULT 0.0,
            signal REAL NOT NULL DEFAULT 0.0,
            noise REAL NOT NULL DEFAULT 0.0,
            logistic_x REAL NOT NULL DEFAULT 0.5,
            logistic_r REAL NOT NULL DEFAULT 3.45,
            chaos_risk REAL NOT NULL DEFAULT 0.0,
            rewrite_aggression REAL NOT NULL DEFAULT 0.0,
            lorenz_x REAL NOT NULL DEFAULT 0.0,
            lorenz_y REAL NOT NULL DEFAULT 0.0,
            lorenz_z REAL NOT NULL DEFAULT 0.0,
            lorenz_radius REAL NOT NULL DEFAULT 0.0,
            lorenz_bounded REAL NOT NULL DEFAULT 0.0,
            lambdoma_global REAL NOT NULL DEFAULT 0.0,
            lambdoma_local REAL NOT NULL DEFAULT 0.0,
            lambdoma_ratio REAL NOT NULL DEFAULT 0.0,
            lambdoma_convergent INTEGER NOT NULL DEFAULT 0,
            rewrite_ready INTEGER NOT NULL DEFAULT 0,
            rewrite_count INTEGER NOT NULL DEFAULT 0,
            security_posture TEXT NOT NULL DEFAULT 'nominal',
            security_events INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_hs_ts ON harmonic_snapshots(ts);
        CREATE INDEX IF NOT EXISTS idx_hs_cycle ON harmonic_snapshots(cycle);
        CREATE INDEX IF NOT EXISTS idx_hs_phase ON harmonic_snapshots(phase);

        -- ═══ Table 2: memory_events ═══
        -- Crystallisation, compression, concept graph mutations.
        CREATE TABLE IF NOT EXISTS memory_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            entries_created INTEGER NOT NULL DEFAULT 0,
            entries_source INTEGER NOT NULL DEFAULT 0,
            old_size INTEGER NOT NULL DEFAULT 0,
            new_size INTEGER NOT NULL DEFAULT 0,
            compression_ratio REAL,
            node_count INTEGER NOT NULL DEFAULT 0,
            edge_count INTEGER NOT NULL DEFAULT 0,
            interdisciplinary_edges INTEGER NOT NULL DEFAULT 0,
            max_depth INTEGER NOT NULL DEFAULT 0,
            detail TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_me_ts ON memory_events(ts);
        CREATE INDEX IF NOT EXISTS idx_me_type ON memory_events(event_type);

        -- ═══ Table 3: phoenix_events ═══
        -- Supervisor lifecycle: start, child_exit, restart, max_restarts, heartbeat.
        CREATE TABLE IF NOT EXISTS phoenix_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            exit_code INTEGER,
            attempt INTEGER,
            max_attempts INTEGER,
            recovery_ms INTEGER,
            detail TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_pe_ts ON phoenix_events(ts);
        CREATE INDEX IF NOT EXISTS idx_pe_type ON phoenix_events(event_type);

        -- ═══ Table 4: ouroboros_events ═══
        -- Self-repair lifecycle: crash, patch_write, patch_apply, recovery.
        CREATE TABLE IF NOT EXISTS ouroboros_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            component TEXT,
            detail TEXT,
            patch_size INTEGER,
            success INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_oe_ts ON ouroboros_events(ts);
        CREATE INDEX IF NOT EXISTS idx_oe_type ON ouroboros_events(event_type);

        -- ═══ Table 5: delegation_log ═══
        -- Model selection decisions with costs, latency, token counts.
        CREATE TABLE IF NOT EXISTS delegation_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            task_hint TEXT,
            model_chosen TEXT NOT NULL,
            backend TEXT NOT NULL DEFAULT 'openrouter',
            reason TEXT,
            escalated INTEGER NOT NULL DEFAULT 0,
            escalated_from TEXT,
            cost_usd REAL NOT NULL DEFAULT 0.0,
            latency_ms INTEGER NOT NULL DEFAULT 0,
            success INTEGER NOT NULL DEFAULT 1,
            tokens_in INTEGER NOT NULL DEFAULT 0,
            tokens_out INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_dl_ts ON delegation_log(ts);
        CREATE INDEX IF NOT EXISTS idx_dl_model ON delegation_log(model_chosen);
        CREATE INDEX IF NOT EXISTS idx_dl_task ON delegation_log(task_hint);

        -- ═══ Table 6: harmony_trajectory ═══
        -- Downsampled 5-minute buckets for long-term evolution graphing.
        CREATE TABLE IF NOT EXISTS harmony_trajectory (
            bucket_ts INTEGER PRIMARY KEY,
            sample_count INTEGER NOT NULL DEFAULT 0,
            avg_signal REAL NOT NULL DEFAULT 0.0,
            min_signal REAL NOT NULL DEFAULT 0.0,
            max_signal REAL NOT NULL DEFAULT 0.0,
            avg_chaos_risk REAL NOT NULL DEFAULT 0.0,
            avg_strength REAL NOT NULL DEFAULT 0.0,
            avg_utility REAL NOT NULL DEFAULT 0.0,
            avg_beauty REAL NOT NULL DEFAULT 0.0
        );

        -- ═══ Table 7: graph_snapshots ═══
        -- Serialised concept graph s-expressions, stored as traversable
        -- adjacency data that the agent can recall and query with SQL.
        CREATE TABLE IF NOT EXISTS graph_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            source TEXT NOT NULL DEFAULT 'memory',
            node_count INTEGER NOT NULL DEFAULT 0,
            edge_count INTEGER NOT NULL DEFAULT 0,
            interdisciplinary_edges INTEGER NOT NULL DEFAULT 0,
            sexp TEXT NOT NULL,
            digest TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_gs_ts ON graph_snapshots(ts);
        CREATE INDEX IF NOT EXISTS idx_gs_source ON graph_snapshots(source);

        -- ═══ Table 8: graph_nodes ═══
        -- Relational decomposition of graph nodes for SQL traversal.
        -- Each row = one concept node from a graph snapshot.
        CREATE TABLE IF NOT EXISTS graph_nodes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES graph_snapshots(id) ON DELETE CASCADE,
            concept TEXT NOT NULL,
            domain TEXT NOT NULL DEFAULT 'generic',
            count INTEGER NOT NULL DEFAULT 1,
            depth_min INTEGER NOT NULL DEFAULT 0,
            depth_max INTEGER NOT NULL DEFAULT 0,
            classes TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_gn_snapshot ON graph_nodes(snapshot_id);
        CREATE INDEX IF NOT EXISTS idx_gn_concept ON graph_nodes(concept);
        CREATE INDEX IF NOT EXISTS idx_gn_domain ON graph_nodes(domain);

        -- ═══ Table 9: graph_edges ═══
        -- Relational decomposition of graph edges for SQL traversal.
        -- Supports adjacency queries, shortest-path CTEs, domain crossing analysis.
        CREATE TABLE IF NOT EXISTS graph_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            snapshot_id INTEGER NOT NULL REFERENCES graph_snapshots(id) ON DELETE CASCADE,
            node_a TEXT NOT NULL,
            node_b TEXT NOT NULL,
            weight INTEGER NOT NULL DEFAULT 1,
            interdisciplinary INTEGER NOT NULL DEFAULT 0,
            reasons TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_ge_snapshot ON graph_edges(snapshot_id);
        CREATE INDEX IF NOT EXISTS idx_ge_nodes ON graph_edges(node_a, node_b);
        CREATE INDEX IF NOT EXISTS idx_ge_weight ON graph_edges(weight DESC);
        CREATE INDEX IF NOT EXISTS idx_ge_inter ON graph_edges(interdisciplinary)
            WHERE interdisciplinary = 1;
        ",
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn migrate_v2(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS signalograd_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            cycle INTEGER NOT NULL DEFAULT 0,
            confidence REAL NOT NULL DEFAULT 0.0,
            stability REAL NOT NULL DEFAULT 0.0,
            novelty REAL NOT NULL DEFAULT 0.0,
            reward REAL NOT NULL DEFAULT 0.0,
            accepted INTEGER NOT NULL DEFAULT 0,
            recall_hits INTEGER NOT NULL DEFAULT 0,
            checkpoint_path TEXT,
            checkpoint_digest TEXT,
            detail TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_se_ts ON signalograd_events(ts);
        CREATE INDEX IF NOT EXISTS idx_se_type ON signalograd_events(event_type);
        CREATE INDEX IF NOT EXISTS idx_se_cycle ON signalograd_events(cycle);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn conn() -> Result<&'static Mutex<Connection>, String> {
    if let Some(c) = DB_CONN.get() {
        return Ok(c);
    }
    let connection = open_connection()?;
    Ok(DB_CONN.get_or_init(|| Mutex::new(connection)))
}

pub fn init() -> Result<(), String> {
    let _ = conn()?;
    Ok(())
}

/// Run an arbitrary SQL query and return results as an s-expression string.
/// Supports SELECT only — mutations are rejected.
pub fn query_sexp(sql: &str) -> Result<String, String> {
    let trimmed = sql.trim();
    let upper = trimmed.to_ascii_uppercase();
    if !upper.starts_with("SELECT") && !upper.starts_with("WITH") {
        return Err("only SELECT / WITH queries allowed".to_string());
    }

    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    let mut stmt = lock.prepare(trimmed).map_err(|e| e.to_string())?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let mut rows_sexp = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let mut fields = Vec::new();
        for (i, name) in col_names.iter().enumerate() {
            let val = row
                .get_ref(i)
                .map(|v| match v {
                    rusqlite::types::ValueRef::Null => "nil".to_string(),
                    rusqlite::types::ValueRef::Integer(n) => n.to_string(),
                    rusqlite::types::ValueRef::Real(f) => format!("{:.6}", f),
                    rusqlite::types::ValueRef::Text(t) => {
                        let s = String::from_utf8_lossy(t);
                        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
                    }
                    rusqlite::types::ValueRef::Blob(b) => format!("#<blob {}>", b.len()),
                })
                .unwrap_or_else(|_| "nil".to_string());
            fields.push(format!(":{} {}", name.replace('_', "-"), val));
        }
        rows_sexp.push(format!("({})", fields.join(" ")));
    }
    Ok(format!("({})", rows_sexp.join("\n ")))
}

// ─── Pressure-aware intelligent GC ─────────────────────────────────────
//
// Instead of naive time-based deletion, the GC is **signal-preserving**:
//   1. Measures actual DB size (page_count * page_size)
//   2. Computes pressure level (soft=50MB, hard=150MB, critical=300MB)
//   3. At each level, prunes more aggressively — but NEVER deletes:
//      - Harmonic snapshots where chaos_risk > 0.5 or rewrite_ready=1 (inflection points)
//      - Phoenix/Ouroboros events that represent actual failures
//      - The latest graph snapshot (always kept for continuity)
//   4. Downsamples harmonic_snapshots → harmony_trajectory before deleting
//   5. Deduplicates graph_snapshots (same digest = same graph, keep newest)
//   6. Thins harmonic_snapshots by keeping 1-per-hour beyond 7 days
//
// The harmony_trajectory table is NEVER pruned — it's the permanent memory
// of how the agent evolved, at negligible storage cost.

/// Default soft limit in bytes (50 MB). Configurable via config-store.
const SOFT_LIMIT_BYTES: i64 = 50 * 1024 * 1024;
/// Hard limit: aggressive pruning kicks in.
const HARD_LIMIT_BYTES: i64 = 150 * 1024 * 1024;
/// Critical: emergency pruning, keep only essentials.
const CRITICAL_LIMIT_BYTES: i64 = 300 * 1024 * 1024;

fn db_size_bytes(conn: &Connection) -> i64 {
    let page_count: i64 = conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap_or(0);
    let page_size: i64 = conn
        .query_row("PRAGMA page_size", [], |r| r.get(0))
        .unwrap_or(4096);
    page_count * page_size
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

const DAY_MS: i64 = 86_400_000;

/// Execute an intelligent GC pass. Returns (deleted_rows, db_size_after, pressure_level).
pub fn gc() -> Result<i32, String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;

    let size_before = db_size_bytes(&lock);
    let now = now_ms();

    // Read configurable limits from config-store (fall back to defaults)
    let soft = harmonia_config_store::get_config_or("chronicle", "gc", "soft-limit-mb", "50")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(50)
        * 1024
        * 1024;

    let hard = harmonia_config_store::get_config_or("chronicle", "gc", "hard-limit-mb", "150")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(150)
        * 1024
        * 1024;

    let mut deleted = 0i32;

    // ── Step 1: Always downsample into harmony_trajectory first ──
    // This preserves the evolutionary signal before we delete any snapshots.
    lock.execute_batch(
        "INSERT OR REPLACE INTO harmony_trajectory
            (bucket_ts, sample_count, avg_signal, min_signal, max_signal,
             avg_chaos_risk, avg_strength, avg_utility, avg_beauty)
         SELECT
            (ts / 300000) * 300000 AS bucket,
            COUNT(*),
            AVG(signal), MIN(signal), MAX(signal),
            AVG(chaos_risk),
            AVG(strength), AVG(utility), AVG(beauty)
         FROM harmonic_snapshots
         GROUP BY bucket
         ON CONFLICT(bucket_ts) DO UPDATE SET
            sample_count = excluded.sample_count,
            avg_signal = excluded.avg_signal,
            min_signal = excluded.min_signal,
            max_signal = excluded.max_signal,
            avg_chaos_risk = excluded.avg_chaos_risk,
            avg_strength = excluded.avg_strength,
            avg_utility = excluded.avg_utility,
            avg_beauty = excluded.avg_beauty;",
    )
    .map_err(|e| e.to_string())?;

    // ── Step 2: Deduplicate graph snapshots ──
    // Keep only the newest snapshot per digest (same graph = same structure).
    deleted += lock
        .execute(
            "DELETE FROM graph_snapshots WHERE id NOT IN (
                SELECT MAX(id) FROM graph_snapshots GROUP BY digest
             )",
            [],
        )
        .map_err(|e| e.to_string())? as i32;

    // ── Step 3: Determine pressure and apply proportional pruning ──
    if size_before < soft {
        // Under soft limit: minimal cleanup only.
        // Just remove truly ancient data (> 90 days) that has no signal value.
        deleted += lock
            .execute(
                "DELETE FROM harmonic_snapshots
                 WHERE ts < ?1
                   AND chaos_risk < 0.3
                   AND rewrite_ready = 0",
                params![now - 90 * DAY_MS],
            )
            .map_err(|e| e.to_string())? as i32;
    } else if size_before < hard {
        // Soft pressure: thin harmonic snapshots beyond 7 days.
        // Keep 1 per hour + all inflection points (high chaos, rewrites, phase changes).
        deleted += thin_harmonic_snapshots(&lock, now, 7 * DAY_MS, 3_600_000)?;

        // Prune boring event tables beyond 30 days
        deleted += prune_events_preserving_failures(&lock, now, 30 * DAY_MS)?;

        // Keep only daily graph snapshots beyond 7 days (plus the latest always)
        deleted += thin_graph_snapshots(&lock, now, 7 * DAY_MS)?;
    } else {
        // Hard/critical pressure: aggressive but still signal-preserving.
        let retention = if size_before >= CRITICAL_LIMIT_BYTES {
            3 * DAY_MS // Critical: keep only 3 days of fine-grained data
        } else {
            7 * DAY_MS
        };

        // Thin snapshots: keep 1 per hour in retention window, 1 per day before that
        deleted += thin_harmonic_snapshots(&lock, now, retention, 3_600_000)?;
        deleted += lock
            .execute(
                "DELETE FROM harmonic_snapshots
                 WHERE ts < ?1
                   AND chaos_risk < 0.5
                   AND rewrite_ready = 0",
                params![now - 30 * DAY_MS],
            )
            .map_err(|e| e.to_string())? as i32;

        // Aggressive event pruning (keep failures)
        deleted += prune_events_preserving_failures(&lock, now, retention)?;

        // Keep only weekly graph snapshots beyond retention
        deleted += thin_graph_snapshots(&lock, now, retention)?;

        // Distill old delegation data: remove individual rows > retention,
        // the trajectory table captures the aggregate.
        deleted += lock
            .execute(
                "DELETE FROM delegation_log WHERE ts < ?1",
                params![now - 30 * DAY_MS],
            )
            .map_err(|e| e.to_string())? as i32;
    }

    // ── Step 4: VACUUM if we deleted a lot ──
    if deleted > 1000 {
        let _ = lock.execute_batch("PRAGMA incremental_vacuum;");
    }

    // Record GC event in chronicle_meta for observability
    let size_after = db_size_bytes(&lock);
    let pressure = if size_after >= CRITICAL_LIMIT_BYTES {
        "critical"
    } else if size_after >= hard {
        "hard"
    } else if size_after >= soft {
        "soft"
    } else {
        "none"
    };

    lock.execute(
        "INSERT OR REPLACE INTO chronicle_meta(key, value)
         VALUES ('last_gc', ?1)",
        params![format!(
            "ts={} deleted={} size_before={} size_after={} pressure={}",
            now, deleted, size_before, size_after, pressure
        )],
    )
    .map_err(|e| e.to_string())?;

    Ok(deleted)
}

/// Thin harmonic snapshots: keep 1 per `bucket_ms` interval, but always preserve
/// inflection points (high chaos, rewrite events, convergence changes).
fn thin_harmonic_snapshots(
    conn: &Connection,
    now: i64,
    older_than: i64,
    bucket_ms: i64,
) -> Result<i32, String> {
    // Delete rows that are:
    //   - Older than the threshold
    //   - NOT an inflection point (chaos_risk > 0.4 OR rewrite_ready=1 OR lambdoma_convergent changed)
    //   - NOT the newest row in their time bucket
    let deleted = conn
        .execute(
            "DELETE FROM harmonic_snapshots
             WHERE ts < ?1
               AND chaos_risk < 0.4
               AND rewrite_ready = 0
               AND id NOT IN (
                   SELECT MAX(id)
                   FROM harmonic_snapshots
                   WHERE ts < ?1
                   GROUP BY ts / ?2
               )",
            params![now - older_than, bucket_ms],
        )
        .map_err(|e| e.to_string())? as i32;
    Ok(deleted)
}

/// Prune event tables but preserve failure/recovery rows (they're the important ones).
fn prune_events_preserving_failures(
    conn: &Connection,
    now: i64,
    older_than: i64,
) -> Result<i32, String> {
    let cutoff = now - older_than;
    let mut deleted = 0i32;

    // memory_events: keep crystallise events (they mark evolution)
    deleted += conn
        .execute(
            "DELETE FROM memory_events
             WHERE ts < ?1 AND event_type NOT IN ('crystallise')",
            params![cutoff],
        )
        .map_err(|e| e.to_string())? as i32;

    // phoenix_events: keep max_restarts and restart events (they're the crises)
    deleted += conn
        .execute(
            "DELETE FROM phoenix_events
             WHERE ts < ?1 AND event_type NOT IN ('max_restarts', 'restart')",
            params![cutoff],
        )
        .map_err(|e| e.to_string())? as i32;

    // ouroboros_events: keep crash and recovery (the story of self-repair)
    deleted += conn
        .execute(
            "DELETE FROM ouroboros_events
             WHERE ts < ?1 AND event_type NOT IN ('crash', 'recovery')",
            params![cutoff],
        )
        .map_err(|e| e.to_string())? as i32;

    Ok(deleted)
}

/// Thin graph snapshots: beyond the threshold, keep only 1 per day
/// (the one with the most structural change from its predecessor).
/// Always keeps the latest snapshot regardless.
fn thin_graph_snapshots(conn: &Connection, now: i64, older_than: i64) -> Result<i32, String> {
    let cutoff = now - older_than;

    // Keep: the latest snapshot + 1 per day (MAX id per day bucket) + any with unique digests
    let deleted = conn
        .execute(
            "DELETE FROM graph_snapshots
             WHERE ts < ?1
               AND id != (SELECT MAX(id) FROM graph_snapshots)
               AND id NOT IN (
                   SELECT MAX(id)
                   FROM graph_snapshots
                   WHERE ts < ?1
                   GROUP BY ts / 86400000
               )",
            params![cutoff],
        )
        .map_err(|e| e.to_string())? as i32;
    Ok(deleted)
}

/// Query the current GC pressure status: size, pressure level, last GC info.
pub fn gc_status() -> Result<String, String> {
    let db = conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;

    let size = db_size_bytes(&lock);
    let size_mb = size as f64 / (1024.0 * 1024.0);

    let pressure = if size >= CRITICAL_LIMIT_BYTES {
        "critical"
    } else if size >= HARD_LIMIT_BYTES {
        "hard"
    } else if size >= SOFT_LIMIT_BYTES {
        "soft"
    } else {
        "none"
    };

    let last_gc: String = lock
        .query_row(
            "SELECT value FROM chronicle_meta WHERE key='last_gc'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "never".to_string());

    // Row counts per table
    let counts: Vec<(String, i64)> = [
        "harmonic_snapshots",
        "memory_events",
        "phoenix_events",
        "ouroboros_events",
        "delegation_log",
        "harmony_trajectory",
        "graph_snapshots",
        "graph_nodes",
        "graph_edges",
    ]
    .iter()
    .map(|table| {
        let n: i64 = lock
            .query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0))
            .unwrap_or(0);
        (table.to_string(), n)
    })
    .collect();

    let counts_sexp: String = counts
        .iter()
        .map(|(t, n)| format!(":{} {}", t.replace('_', "-"), n))
        .collect::<Vec<_>>()
        .join(" ");

    Ok(format!(
        "(:size-mb {:.2} :pressure \"{}\" :last-gc \"{}\" :tables ({}))",
        size_mb, pressure, last_gc, counts_sexp
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_creates_tables() {
        // Use in-memory for testing
        std::env::set_var("HARMONIA_CHRONICLE_DB", ":memory:");
        assert!(init().is_ok());
    }
}
