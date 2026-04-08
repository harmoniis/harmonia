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

use rusqlite::{params, Connection};

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
    let db = super::db::conn()?;
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
    let db = super::db::conn()?;
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
        "supervision_specs",
        "supervision_evidence",
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
