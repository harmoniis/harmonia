use std::path::PathBuf;

use rusqlite::{params, Connection};

use crate::model::{Edge, MatrixEvent, RouteSample, State, StoreConfig};

use super::super::shared::{history_limit, push_limited};

#[allow(dead_code)]
fn sqlite_conn(path: &str) -> Result<Connection, String> {
    let db_path = PathBuf::from(path);
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("matrix db dir create failed: {e}"))?;
    }
    let conn = Connection::open(db_path).map_err(|e| format!("matrix db open failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS meta (k TEXT PRIMARY KEY NOT NULL, v TEXT NOT NULL)",
        [],
    )
    .map_err(|e| format!("matrix db schema meta failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS nodes (id TEXT PRIMARY KEY NOT NULL, kind TEXT NOT NULL)",
        [],
    )
    .map_err(|e| format!("matrix db schema nodes failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS tools (id TEXT PRIMARY KEY NOT NULL, enabled INTEGER NOT NULL)",
        [],
    )
    .map_err(|e| format!("matrix db schema tools failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS edges (
            src TEXT NOT NULL,
            dst TEXT NOT NULL,
            weight REAL NOT NULL,
            min_harmony REAL NOT NULL,
            uses INTEGER NOT NULL,
            successes INTEGER NOT NULL,
            total_latency_ms INTEGER NOT NULL,
            total_cost_usd REAL NOT NULL,
            PRIMARY KEY (src, dst)
        )",
        [],
    )
    .map_err(|e| format!("matrix db schema edges failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS route_samples (
            src TEXT NOT NULL,
            dst TEXT NOT NULL,
            ts INTEGER NOT NULL,
            success INTEGER NOT NULL,
            latency_ms INTEGER NOT NULL,
            cost_usd REAL NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("matrix db schema route_samples failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS events (
            ts INTEGER NOT NULL,
            component TEXT NOT NULL,
            direction TEXT NOT NULL,
            channel TEXT NOT NULL,
            payload TEXT NOT NULL,
            success INTEGER NOT NULL,
            error TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("matrix db schema events failed: {e}"))?;
    Ok(conn)
}

#[allow(dead_code)]
pub(super) fn persist_state_sqlite(st: &State, cfg: &StoreConfig) -> Result<(), String> {
    let conn = sqlite_conn(&cfg.path)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| format!("matrix db tx begin failed: {e}"))?;

    tx.execute("DELETE FROM meta", [])
        .map_err(|e| format!("matrix db clear meta failed: {e}"))?;
    tx.execute("DELETE FROM nodes", [])
        .map_err(|e| format!("matrix db clear nodes failed: {e}"))?;
    tx.execute("DELETE FROM tools", [])
        .map_err(|e| format!("matrix db clear tools failed: {e}"))?;
    tx.execute("DELETE FROM edges", [])
        .map_err(|e| format!("matrix db clear edges failed: {e}"))?;
    tx.execute("DELETE FROM route_samples", [])
        .map_err(|e| format!("matrix db clear route_samples failed: {e}"))?;
    tx.execute("DELETE FROM events", [])
        .map_err(|e| format!("matrix db clear events failed: {e}"))?;

    tx.execute(
        "INSERT INTO meta(k, v) VALUES ('epoch', ?1)",
        params![st.epoch.to_string()],
    )
    .map_err(|e| format!("matrix db write epoch failed: {e}"))?;
    tx.execute(
        "INSERT INTO meta(k, v) VALUES ('revision', ?1)",
        params![st.revision.to_string()],
    )
    .map_err(|e| format!("matrix db write revision failed: {e}"))?;

    for (id, kind) in &st.nodes {
        tx.execute(
            "INSERT INTO nodes(id, kind) VALUES (?1, ?2)",
            params![id, kind],
        )
        .map_err(|e| format!("matrix db write node failed: {e}"))?;
    }

    for (id, enabled) in &st.plugged {
        tx.execute(
            "INSERT INTO tools(id, enabled) VALUES (?1, ?2)",
            params![id, if *enabled { 1 } else { 0 }],
        )
        .map_err(|e| format!("matrix db write tool failed: {e}"))?;
    }

    for ((src, dst), edge) in &st.edges {
        tx.execute(
            "INSERT INTO edges(src, dst, weight, min_harmony, uses, successes, total_latency_ms, total_cost_usd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                src,
                dst,
                edge.weight,
                edge.min_harmony,
                edge.uses,
                edge.successes,
                edge.total_latency_ms,
                edge.total_cost_usd
            ],
        )
        .map_err(|e| format!("matrix db write edge failed: {e}"))?;
    }

    for ((src, dst), samples) in &st.route_history {
        for s in samples {
            tx.execute(
                "INSERT INTO route_samples(src, dst, ts, success, latency_ms, cost_usd)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    src,
                    dst,
                    s.ts,
                    if s.success { 1 } else { 0 },
                    s.latency_ms,
                    s.cost_usd
                ],
            )
            .map_err(|e| format!("matrix db write route sample failed: {e}"))?;
        }
    }

    for e in &st.events {
        tx.execute(
            "INSERT INTO events(ts, component, direction, channel, payload, success, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                e.ts,
                e.component,
                e.direction,
                e.channel,
                e.payload,
                if e.success { 1 } else { 0 },
                e.error
            ],
        )
        .map_err(|e| format!("matrix db write event failed: {e}"))?;
    }

    tx.commit()
        .map_err(|e| format!("matrix db tx commit failed: {e}"))?;
    Ok(())
}

#[allow(dead_code)]
pub(super) fn load_state_sqlite(cfg: &StoreConfig) -> Result<Option<State>, String> {
    let conn = sqlite_conn(&cfg.path)?;

    let has_rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM nodes", [], |row| row.get(0))
        .map_err(|e| format!("matrix db count nodes failed: {e}"))?;

    if has_rows == 0 {
        return Ok(None);
    }

    let mut st = State::default();

    let epoch: String = conn
        .query_row("SELECT v FROM meta WHERE k='epoch' LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or_else(|_| "0".to_string());
    let revision: String = conn
        .query_row("SELECT v FROM meta WHERE k='revision' LIMIT 1", [], |row| {
            row.get(0)
        })
        .unwrap_or_else(|_| "0".to_string());
    st.epoch = epoch.parse::<u64>().unwrap_or(0);
    st.revision = revision.parse::<u64>().unwrap_or(0);

    {
        let mut stmt = conn
            .prepare("SELECT id, kind FROM nodes")
            .map_err(|e| format!("matrix db read nodes prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let kind: String = row.get(1)?;
                Ok((id, kind))
            })
            .map_err(|e| format!("matrix db read nodes query failed: {e}"))?;
        for (id, kind) in rows.flatten() {
            st.nodes.insert(id, kind);
        }
    }

    {
        let mut stmt = conn
            .prepare("SELECT id, enabled FROM tools")
            .map_err(|e| format!("matrix db read tools prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let enabled: i64 = row.get(1)?;
                Ok((id, enabled != 0))
            })
            .map_err(|e| format!("matrix db read tools query failed: {e}"))?;
        for (id, enabled) in rows.flatten() {
            st.plugged.insert(id, enabled);
        }
    }

    {
        let mut stmt = conn
            .prepare(
                "SELECT src, dst, weight, min_harmony, uses, successes, total_latency_ms, total_cost_usd FROM edges",
            )
            .map_err(|e| format!("matrix db read edges prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let src: String = row.get(0)?;
                let dst: String = row.get(1)?;
                let edge = Edge {
                    weight: row.get(2)?,
                    min_harmony: row.get(3)?,
                    uses: row.get(4)?,
                    successes: row.get(5)?,
                    total_latency_ms: row.get(6)?,
                    total_cost_usd: row.get(7)?,
                };
                Ok((src, dst, edge))
            })
            .map_err(|e| format!("matrix db read edges query failed: {e}"))?;
        for (src, dst, edge) in rows.flatten() {
            st.edges.insert((src, dst), edge);
        }
    }

    {
        let mut stmt = conn
            .prepare("SELECT src, dst, ts, success, latency_ms, cost_usd FROM route_samples ORDER BY ts ASC")
            .map_err(|e| format!("matrix db read route_samples prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let src: String = row.get(0)?;
                let dst: String = row.get(1)?;
                let sample = RouteSample {
                    ts: row.get(2)?,
                    success: {
                        let v: i64 = row.get(3)?;
                        v != 0
                    },
                    latency_ms: row.get(4)?,
                    cost_usd: row.get(5)?,
                };
                Ok((src, dst, sample))
            })
            .map_err(|e| format!("matrix db read route_samples query failed: {e}"))?;

        let limit = history_limit();
        for (src, dst, sample) in rows.flatten() {
            let k = (src, dst);
            let samples = st.route_history.entry(k).or_default();
            push_limited(samples, sample, limit);
        }
    }

    {
        let mut stmt = conn
            .prepare("SELECT ts, component, direction, channel, payload, success, error FROM events ORDER BY ts ASC")
            .map_err(|e| format!("matrix db read events prepare failed: {e}"))?;
        let rows = stmt
            .query_map([], |row| {
                let success: i64 = row.get(5)?;
                Ok(MatrixEvent {
                    ts: row.get(0)?,
                    component: row.get(1)?,
                    direction: row.get(2)?,
                    channel: row.get(3)?,
                    payload: row.get(4)?,
                    success: success != 0,
                    error: row.get(6)?,
                })
            })
            .map_err(|e| format!("matrix db read events query failed: {e}"))?;
        let limit = history_limit();
        for e in rows.flatten() {
            push_limited(&mut st.events, e, limit);
        }
    }

    Ok(Some(st))
}
