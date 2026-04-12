use rusqlite::Connection;

pub(crate) fn migrate_v2(conn: &Connection) -> Result<(), String> {
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

pub(crate) fn migrate_v3(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        -- ═══ Supervision specs ═══
        -- Frozen before task execution. Verdict filled after completion.
        CREATE TABLE IF NOT EXISTS supervision_specs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            task INTEGER NOT NULL,
            taxonomy TEXT NOT NULL DEFAULT 'deferred',
            spec TEXT NOT NULL,
            assertions INTEGER NOT NULL DEFAULT 0,
            passed INTEGER DEFAULT NULL,
            failed INTEGER DEFAULT NULL,
            skipped INTEGER DEFAULT NULL,
            verdict TEXT DEFAULT NULL,
            confidence REAL DEFAULT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ss_ts ON supervision_specs(ts);
        CREATE INDEX IF NOT EXISTS idx_ss_task ON supervision_specs(task);
        CREATE INDEX IF NOT EXISTS idx_ss_verdict ON supervision_specs(verdict);

        -- ═══ Supervision evidence ═══
        -- Individual assertion evaluations linked to a spec.
        CREATE TABLE IF NOT EXISTS supervision_evidence (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            spec INTEGER NOT NULL REFERENCES supervision_specs(id),
            kind TEXT NOT NULL,
            detail TEXT,
            passed INTEGER NOT NULL DEFAULT 0,
            evidence TEXT,
            duration INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_sve_spec ON supervision_evidence(spec);
        CREATE INDEX IF NOT EXISTS idx_sve_kind ON supervision_evidence(kind);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn migrate_v4(conn: &Connection) -> Result<(), String> {
    // Memory-field basin state columns for warm-start across restarts.
    // Each ALTER is separate so already-existing columns don't block others.
    let columns = [
        "ALTER TABLE harmonic_snapshots ADD COLUMN field_basin TEXT NOT NULL DEFAULT 'thomas-0'",
        "ALTER TABLE harmonic_snapshots ADD COLUMN field_coercive_energy REAL NOT NULL DEFAULT 0.0",
        "ALTER TABLE harmonic_snapshots ADD COLUMN field_dwell_ticks INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE harmonic_snapshots ADD COLUMN field_threshold REAL NOT NULL DEFAULT 0.35",
    ];
    for sql in &columns {
        let _ = conn.execute_batch(sql); // Ignore "duplicate column" errors.
    }
    Ok(())
}

pub(crate) fn migrate_v5(conn: &Connection) -> Result<(), String> {
    // Persistent memory entries — the agent's living memory.
    // No fixed categories. Tags are freeform. The field's topology is the classification.
    // Dedup via content_hash — same content never stored twice.
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS memory_entries (
            id TEXT PRIMARY KEY,
            ts INTEGER NOT NULL,
            content TEXT NOT NULL,
            tags TEXT DEFAULT '',
            source_ids TEXT DEFAULT '',
            access_count INTEGER DEFAULT 0,
            last_access INTEGER,
            content_hash TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_me_ts ON memory_entries(ts);
        CREATE INDEX IF NOT EXISTS idx_me_hash ON memory_entries(content_hash);
        CREATE INDEX IF NOT EXISTS idx_me_access ON memory_entries(access_count DESC);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn migrate_v6(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS terraphon_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            lode_id TEXT NOT NULL,
            platform TEXT NOT NULL,
            node_label TEXT NOT NULL,
            domain TEXT,
            strategy TEXT,
            elapsed_ms INTEGER DEFAULT 0,
            result_size INTEGER DEFAULT 0,
            compressed INTEGER DEFAULT 0,
            cross_node INTEGER DEFAULT 0,
            origin_node TEXT,
            error TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_te_ts ON terraphon_events(ts);
        CREATE INDEX IF NOT EXISTS idx_te_lode ON terraphon_events(lode_id);

        CREATE TABLE IF NOT EXISTS palace_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            event_type TEXT NOT NULL,
            operation TEXT NOT NULL,
            node_id INTEGER,
            label TEXT,
            detail TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_pe_ts ON palace_events(ts);
        CREATE INDEX IF NOT EXISTS idx_pe_type ON palace_events(event_type);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn migrate_v7(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS error_events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
            source TEXT NOT NULL,
            kind TEXT NOT NULL,
            model TEXT DEFAULT '',
            detail TEXT DEFAULT '',
            latency_ms INTEGER DEFAULT 0,
            cascaded_to TEXT DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_ee_ts ON error_events(ts);
        CREATE INDEX IF NOT EXISTS idx_ee_source ON error_events(source);
        CREATE INDEX IF NOT EXISTS idx_ee_kind ON error_events(kind);
        CREATE INDEX IF NOT EXISTS idx_ee_model ON error_events(model);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn migrate_v8(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS palace_drawers (
            id INTEGER PRIMARY KEY,
            content TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'conversation',
            room_id INTEGER NOT NULL DEFAULT 0,
            chunk_index INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            tags TEXT DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_pd_room ON palace_drawers(room_id);
        CREATE INDEX IF NOT EXISTS idx_pd_source ON palace_drawers(source);

        CREATE TABLE IF NOT EXISTS palace_nodes (
            id INTEGER PRIMARY KEY,
            kind TEXT NOT NULL DEFAULT 'concept',
            label TEXT NOT NULL,
            domain TEXT NOT NULL DEFAULT 'generic',
            created_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_pn_label ON palace_nodes(label);
        CREATE INDEX IF NOT EXISTS idx_pn_domain ON palace_nodes(domain);

        CREATE TABLE IF NOT EXISTS palace_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id INTEGER NOT NULL,
            target_id INTEGER NOT NULL,
            kind TEXT NOT NULL DEFAULT 'relates_to',
            weight REAL NOT NULL DEFAULT 1.0,
            confidence REAL NOT NULL DEFAULT 1.0
        );
        CREATE INDEX IF NOT EXISTS idx_pe2_src ON palace_edges(source_id);
        CREATE INDEX IF NOT EXISTS idx_pe2_tgt ON palace_edges(target_id);
        ",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn migrate_v9(conn: &Connection) -> Result<(), String> {
    // Full memory-field checkpoint sexp stored per harmonic cycle for warm-start.
    // ALTER is separate so already-existing column doesn't block.
    let _ = conn.execute_batch(
        "ALTER TABLE harmonic_snapshots ADD COLUMN field_checkpoint TEXT NOT NULL DEFAULT ''",
    );
    Ok(())
}
