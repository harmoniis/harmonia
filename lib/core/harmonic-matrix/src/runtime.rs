use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};

use crate::model::{Edge, MatrixEvent, RouteSample, State, StoreConfig, StoreKind};

static STATE: OnceLock<RwLock<State>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
static STORE_CONFIG: OnceLock<RwLock<StoreConfig>> = OnceLock::new();

fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| RwLock::new(State::default()))
}

fn last_error_slot() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn store_config() -> &'static RwLock<StoreConfig> {
    STORE_CONFIG.get_or_init(|| RwLock::new(StoreConfig::default()))
}

pub(crate) fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_slot().write() {
        *slot = msg.into();
    }
}

pub(crate) fn clear_last_error() {
    if let Ok(mut slot) = last_error_slot().write() {
        slot.clear();
    }
}

pub(crate) fn last_error_message() -> String {
    last_error_slot()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "harmonic matrix error lock poisoned".to_string())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn history_limit() -> usize {
    std::env::var("HARMONIA_MATRIX_HISTORY_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(4096)
}

fn truncate_payload(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect::<String>()
}

fn push_limited<T>(v: &mut Vec<T>, item: T, limit: usize) {
    v.push(item);
    if v.len() > limit {
        let over = v.len() - limit;
        v.drain(0..over);
    }
}

fn tool_allowed(st: &State, node_id: &str) -> bool {
    if st.nodes.get(node_id).map(|k| k.as_str()) != Some("tool") {
        return true;
    }
    st.plugged.get(node_id).copied().unwrap_or(true)
}

fn bump_revision(st: &mut State) {
    st.revision = st.revision.saturating_add(1);
}

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

fn persist_state_sqlite(st: &State, cfg: &StoreConfig) -> Result<(), String> {
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

fn load_state_sqlite(cfg: &StoreConfig) -> Result<Option<State>, String> {
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

fn persist_if_needed(st: &State) -> Result<(), String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();
    match cfg.kind {
        StoreKind::Memory => Ok(()),
        StoreKind::Sqlite => persist_state_sqlite(st, &cfg),
        StoreKind::Graph => Err("graph store adapter contract exists but implementation is pending; use memory|sqlite for now".to_string()),
    }
}

pub(crate) fn set_store(kind: &str, path_override: Option<&str>) -> Result<(), String> {
    let parsed = if kind.eq_ignore_ascii_case("memory") {
        StoreKind::Memory
    } else if kind.eq_ignore_ascii_case("sqlite") || kind.eq_ignore_ascii_case("sql") {
        StoreKind::Sqlite
    } else if kind.eq_ignore_ascii_case("graph") {
        StoreKind::Graph
    } else {
        return Err("invalid store kind, expected memory|sqlite|graph".to_string());
    };

    if parsed == StoreKind::Graph {
        return Err(
            "graph store adapter contract exists but implementation is pending; use memory|sqlite for now"
                .to_string(),
        );
    }

    let mut cfg = store_config()
        .write()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?;
    cfg.kind = parsed;
    if let Some(path) = path_override {
        if !path.trim().is_empty() {
            cfg.path = path.to_string();
        }
    }
    let cfg_now = cfg.clone();
    drop(cfg);

    if cfg_now.kind == StoreKind::Sqlite {
        let mut st = state()
            .write()
            .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
        if let Some(loaded) = load_state_sqlite(&cfg_now)? {
            *st = loaded;
        } else {
            persist_state_sqlite(&st, &cfg_now)?;
        }
    }

    Ok(())
}

pub(crate) fn init() -> Result<(), String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    if cfg.kind == StoreKind::Sqlite {
        match load_state_sqlite(&cfg)? {
            Some(loaded) => {
                *st = loaded;
                return Ok(());
            }
            None => {}
        }
    } else if cfg.kind == StoreKind::Graph {
        return Err("graph store adapter contract exists but implementation is pending; use memory|sqlite for now".to_string());
    }

    st.nodes.clear();
    st.edges.clear();
    st.plugged.clear();
    st.route_history.clear();
    st.events.clear();
    st.epoch = now_unix();
    st.revision = 1;
    persist_if_needed(&st)?;
    Ok(())
}

pub(crate) fn store_summary() -> Result<String, String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();
    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\")",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\"")
    ))
}

pub(crate) fn register_node(node_id: &str, kind: &str) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    st.nodes.insert(node_id.to_string(), kind.to_string());
    if kind == "tool" {
        st.plugged.entry(node_id.to_string()).or_insert(true);
    }
    bump_revision(&mut st);
    persist_if_needed(&st)
}

pub(crate) fn set_tool_enabled(tool_id: &str, enabled: bool) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    match st.nodes.get(tool_id) {
        Some(kind) if kind == "tool" => {
            st.plugged.insert(tool_id.to_string(), enabled);
            bump_revision(&mut st);
            persist_if_needed(&st)
        }
        _ => Err("tool not registered or not kind=tool".to_string()),
    }
}

pub(crate) fn register_edge(
    from: &str,
    to: &str,
    weight: f64,
    min_harmony: f64,
) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    if !st.nodes.contains_key(from) || !st.nodes.contains_key(to) {
        return Err("both nodes must be registered before edge registration".to_string());
    }

    st.edges.insert(
        (from.to_string(), to.to_string()),
        Edge {
            weight,
            min_harmony,
            uses: 0,
            successes: 0,
            total_latency_ms: 0,
            total_cost_usd: 0.0,
        },
    );

    bump_revision(&mut st);
    persist_if_needed(&st)
}

pub(crate) fn route_allowed(from: &str, to: &str, signal: f64, noise: f64) -> Result<bool, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let edge = st
        .edges
        .get(&(from.to_string(), to.to_string()))
        .ok_or_else(|| format!("route denied: edge missing {} -> {}", from, to))?;

    if !tool_allowed(&st, from) || !tool_allowed(&st, to) {
        return Err(format!(
            "route denied: unplugged tool on {} -> {}",
            from, to
        ));
    }

    let harmonic_signal = signal - noise + edge.weight;
    let allowed = signal >= noise && harmonic_signal >= edge.min_harmony;
    if !allowed {
        return Err(format!(
            "route denied by harmonic threshold {} -> {} (signal={:.4} noise={:.4} weight={:.4} min={:.4})",
            from, to, signal, noise, edge.weight, edge.min_harmony
        ));
    }

    Ok(true)
}

pub(crate) fn observe_route(
    from: &str,
    to: &str,
    success: bool,
    latency_ms: u64,
    cost_usd: f64,
) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let edge = st
        .edges
        .get_mut(&(from.to_string(), to.to_string()))
        .ok_or_else(|| format!("route observe failed: edge missing {} -> {}", from, to))?;

    edge.uses += 1;
    if success {
        edge.successes += 1;
    }
    edge.total_latency_ms += latency_ms;
    edge.total_cost_usd += cost_usd.max(0.0);

    let key = (from.to_string(), to.to_string());
    let sample = RouteSample {
        ts: now_unix(),
        success,
        latency_ms,
        cost_usd: cost_usd.max(0.0),
    };
    let limit = history_limit();
    let samples = st.route_history.entry(key).or_default();
    push_limited(samples, sample, limit);
    bump_revision(&mut st);

    persist_if_needed(&st)
}

pub(crate) fn log_event(
    component: &str,
    direction: &str,
    channel: &str,
    payload: &str,
    success: bool,
    error: &str,
) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let event = MatrixEvent {
        ts: now_unix(),
        component: component.to_string(),
        direction: direction.to_string(),
        channel: channel.to_string(),
        payload: truncate_payload(payload, 512),
        success,
        error: truncate_payload(error, 512),
    };
    let limit = history_limit();
    push_limited(&mut st.events, event, limit);
    bump_revision(&mut st);

    persist_if_needed(&st)
}

pub(crate) fn route_timeseries(from: &str, to: &str, limit: i32) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let samples = st
        .route_history
        .get(&(from.to_string(), to.to_string()))
        .cloned()
        .unwrap_or_default();

    let n = if limit <= 0 {
        samples.len()
    } else {
        limit as usize
    };
    let start = samples.len().saturating_sub(n);
    let out: Vec<String> = samples[start..]
        .iter()
        .map(|s| {
            format!(
                "(:ts {} :success {} :latency-ms {} :cost-usd {:.8})",
                s.ts,
                if s.success { "t" } else { "nil" },
                s.latency_ms,
                s.cost_usd
            )
        })
        .collect();

    Ok(format!(
        "(:from \"{}\" :to \"{}\" :samples ({}))",
        from,
        to,
        out.join(" ")
    ))
}

pub(crate) fn time_report(since_unix: u64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let events: Vec<&MatrixEvent> = st.events.iter().filter(|e| e.ts >= since_unix).collect();
    let mut by_component: HashMap<String, (u64, u64)> = HashMap::new();
    for e in &events {
        let entry = by_component.entry(e.component.clone()).or_insert((0, 0));
        entry.0 += 1;
        if e.success {
            entry.1 += 1;
        }
    }

    let mut components = Vec::new();
    for (c, (count, ok)) in by_component {
        let sr = if count == 0 {
            0.0
        } else {
            ok as f64 / count as f64
        };
        components.push(format!(
            "(:component \"{}\" :count {} :success-rate {:.4})",
            c, count, sr
        ));
    }
    components.sort();

    let mut recent_events = Vec::new();
    for e in events.iter().rev().take(20).rev() {
        recent_events.push(format!(
            "(:ts {} :component \"{}\" :direction \"{}\" :channel \"{}\" :success {} :payload \"{}\" :error \"{}\")",
            e.ts,
            e.component,
            e.direction,
            e.channel,
            if e.success { "t" } else { "nil" },
            e.payload.replace('"', "\\\""),
            e.error.replace('"', "\\\"")
        ));
    }

    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\" :epoch {} :revision {} :since {} :event-count {} :components ({}) :recent-events ({}))",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\""),
        st.epoch,
        st.revision,
        since_unix,
        events.len(),
        components.join(" "),
        recent_events.join(" ")
    ))
}

pub(crate) fn report() -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let mut node_entries: Vec<String> = st
        .nodes
        .iter()
        .map(|(id, kind)| {
            let plugged = if kind == "tool" {
                st.plugged.get(id).copied().unwrap_or(true)
            } else {
                true
            };
            format!(
                "(:id \"{}\" :kind :{} :plugged {})",
                id,
                kind,
                if plugged { "t" } else { "nil" }
            )
        })
        .collect();
    node_entries.sort();

    let mut edge_entries: Vec<String> = st
        .edges
        .iter()
        .map(|((from, to), e)| {
            let sr = if e.uses == 0 {
                0.0
            } else {
                e.successes as f64 / e.uses as f64
            };
            let avg_latency = if e.uses == 0 {
                0.0
            } else {
                e.total_latency_ms as f64 / e.uses as f64
            };
            let hist = st
                .route_history
                .get(&(from.clone(), to.clone()))
                .map(|h| h.len())
                .unwrap_or(0);
            format!(
                "(:from \"{}\" :to \"{}\" :weight {:.4} :min-harmony {:.4} :uses {} :success-rate {:.4} :avg-latency-ms {:.2} :total-cost-usd {:.8} :history {})",
                from, to, e.weight, e.min_harmony, e.uses, sr, avg_latency, e.total_cost_usd, hist
            )
        })
        .collect();
    edge_entries.sort();

    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\" :epoch {} :revision {} :event-count {} :nodes ({}) :edges ({}))",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\""),
        st.epoch,
        st.revision,
        st.events.len(),
        node_entries.join(" "),
        edge_entries.join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .expect("matrix test lock poisoned")
    }

    #[test]
    fn sqlite_roundtrip_persists_usage() {
        let _guard = test_guard();
        let db = "/tmp/harmonia/hmatrix-modtest.db";
        let _ = std::fs::remove_file(db);

        set_store("sqlite", Some(db)).expect("set store sqlite");
        init().expect("init");
        register_node("a", "core").expect("node a");
        register_node("b", "tool").expect("node b");
        register_edge("a", "b", 1.0, 0.1).expect("edge");
        set_tool_enabled("b", true).expect("tool");
        observe_route("a", "b", true, 10, 0.01).expect("observe");
        log_event("a", "output", "test", "payload", true, "").expect("event");

        let r1 = report().expect("report1");
        assert!(r1.contains(":uses 1"));

        init().expect("re-init");
        let r2 = report().expect("report2");
        assert!(r2.contains(":uses 1"));
        assert!(r2.contains(":store-kind \"sqlite\""));

        set_store("memory", None).expect("back to memory");
    }

    #[test]
    fn graph_store_contract_returns_error() {
        let _guard = test_guard();
        set_store("memory", None).expect("start on memory");
        let err = set_store("graph", Some("bolt://127.0.0.1:7687"))
            .expect_err("graph should not be active yet");
        assert!(err.contains("contract exists"));
        let summary = store_summary().expect("summary after graph failure");
        assert!(summary.contains(":store-kind \"memory\""));
    }
}
