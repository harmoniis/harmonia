use rusqlite::{params, Connection};

// ─── Schema version for migrations ────────────────────────────────────
pub(crate) const SCHEMA_VERSION: i32 = 7;

pub(crate) fn run_migrations(conn: &Connection) -> Result<(), String> {
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
        super::migrations::migrate_v2(conn)?;
    }
    if current_version < 3 {
        super::migrations::migrate_v3(conn)?;
    }
    if current_version < 4 {
        super::migrations::migrate_v4(conn)?;
    }
    if current_version < 5 {
        super::migrations::migrate_v5(conn)?;
    }
    if current_version < 6 {
        super::migrations::migrate_v6(conn)?;
    }
    if current_version < 7 {
        super::migrations::migrate_v7(conn)?;
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
