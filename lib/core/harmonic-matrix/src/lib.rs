use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-harmonic-matrix/0.2.0\0";

#[derive(Clone, Debug)]
struct Edge {
    weight: f64,
    min_harmony: f64,
    uses: u64,
    successes: u64,
    total_latency_ms: u64,
    total_cost_usd: f64,
}

#[derive(Clone, Debug)]
struct RouteSample {
    ts: u64,
    success: bool,
    latency_ms: u64,
    cost_usd: f64,
}

#[derive(Clone, Debug)]
struct MatrixEvent {
    ts: u64,
    component: String,
    direction: String,
    channel: String,
    payload: String,
    success: bool,
    error: String,
}

#[derive(Default)]
struct State {
    nodes: HashMap<String, String>,
    edges: HashMap<(String, String), Edge>,
    plugged: HashMap<String, bool>,
    route_history: HashMap<(String, String), Vec<RouteSample>>,
    events: Vec<MatrixEvent>,
    epoch: u64,
    revision: u64,
}

static STATE: OnceLock<RwLock<State>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| RwLock::new(State::default()))
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn cstr_to_optional_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Some(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn history_limit() -> usize {
    env::var("HARMONIA_MATRIX_HISTORY_LIMIT")
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

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_init() -> i32 {
    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };
    st.nodes.clear();
    st.edges.clear();
    st.plugged.clear();
    st.route_history.clear();
    st.events.clear();
    st.epoch = now_unix();
    st.revision = 1;
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_register_node(
    node_id: *const c_char,
    kind: *const c_char,
) -> i32 {
    let node_id = match cstr_to_string(node_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };

    st.nodes.insert(node_id.clone(), kind.clone());
    if kind == "tool" {
        st.plugged.entry(node_id).or_insert(true);
    }
    bump_revision(&mut st);
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_set_tool_enabled(
    tool_id: *const c_char,
    enabled: i32,
) -> i32 {
    let tool_id = match cstr_to_string(tool_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };

    match st.nodes.get(&tool_id) {
        Some(kind) if kind == "tool" => {
            st.plugged.insert(tool_id, enabled != 0);
            bump_revision(&mut st);
            clear_error();
            0
        }
        _ => {
            set_error("tool not registered or not kind=tool");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_register_edge(
    from: *const c_char,
    to: *const c_char,
    weight: f64,
    min_harmony: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };

    if !st.nodes.contains_key(&from) || !st.nodes.contains_key(&to) {
        set_error("both nodes must be registered before edge registration");
        return -1;
    }

    st.edges.insert(
        (from, to),
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
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_route_allowed(
    from: *const c_char,
    to: *const c_char,
    signal: f64,
    noise: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return 0;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return 0;
        }
    };

    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return 0;
        }
    };

    let edge = match st.edges.get(&(from.clone(), to.clone())) {
        Some(v) => v,
        None => {
            set_error(format!("route denied: edge missing {} -> {}", from, to));
            return 0;
        }
    };

    if !tool_allowed(&st, &from) || !tool_allowed(&st, &to) {
        set_error(format!("route denied: unplugged tool on {} -> {}", from, to));
        return 0;
    }

    let harmonic_signal = signal - noise + edge.weight;
    let allowed = signal >= noise && harmonic_signal >= edge.min_harmony;
    if !allowed {
        set_error(format!(
            "route denied by harmonic threshold {} -> {} (signal={:.4} noise={:.4} weight={:.4} min={:.4})",
            from, to, signal, noise, edge.weight, edge.min_harmony
        ));
        return 0;
    }

    clear_error();
    1
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_observe_route(
    from: *const c_char,
    to: *const c_char,
    success: i32,
    latency_ms: u64,
    cost_usd: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };

    let edge = match st.edges.get_mut(&(from.clone(), to.clone())) {
        Some(v) => v,
        None => {
            set_error(format!("route observe failed: edge missing {} -> {}", from, to));
            return -1;
        }
    };

    edge.uses += 1;
    if success != 0 {
        edge.successes += 1;
    }
    edge.total_latency_ms += latency_ms;
    edge.total_cost_usd += cost_usd.max(0.0);

    let key = (from, to);
    let sample = RouteSample {
        ts: now_unix(),
        success: success != 0,
        latency_ms,
        cost_usd: cost_usd.max(0.0),
    };
    let limit = history_limit();
    let samples = st.route_history.entry(key).or_default();
    push_limited(samples, sample, limit);
    bump_revision(&mut st);

    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_log_event(
    component: *const c_char,
    direction: *const c_char,
    channel: *const c_char,
    payload: *const c_char,
    success: i32,
    error: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let direction = match cstr_to_string(direction) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let channel = match cstr_to_string(channel) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let payload = cstr_to_optional_string(payload).unwrap_or_default();
    let error = cstr_to_optional_string(error).unwrap_or_default();

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return -1;
        }
    };

    let event = MatrixEvent {
        ts: now_unix(),
        component,
        direction,
        channel,
        payload: truncate_payload(&payload, 512),
        success: success != 0,
        error: truncate_payload(&error, 512),
    };
    let limit = history_limit();
    push_limited(&mut st.events, event, limit);
    bump_revision(&mut st);
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_route_timeseries(
    from: *const c_char,
    to: *const c_char,
    limit: i32,
) -> *mut c_char {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return std::ptr::null_mut();
        }
    };

    let samples = st
        .route_history
        .get(&(from.clone(), to.clone()))
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

    clear_error();
    to_c_string(format!(
        "(:from \"{}\" :to \"{}\" :samples ({}))",
        from,
        to,
        out.join(" ")
    ))
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_time_report(since_unix: u64) -> *mut c_char {
    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return std::ptr::null_mut();
        }
    };

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
        let sr = if count == 0 { 0.0 } else { ok as f64 / count as f64 };
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

    clear_error();
    to_c_string(format!(
        "(:epoch {} :revision {} :since {} :event-count {} :components ({}) :recent-events ({}))",
        st.epoch,
        st.revision,
        since_unix,
        events.len(),
        components.join(" "),
        recent_events.join(" ")
    ))
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_report() -> *mut c_char {
    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("harmonic matrix state lock poisoned");
            return std::ptr::null_mut();
        }
    };

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
            let hist = st.route_history.get(&(from.clone(), to.clone())).map(|h| h.len()).unwrap_or(0);
            format!(
                "(:from \"{}\" :to \"{}\" :weight {:.4} :min-harmony {:.4} :uses {} :success-rate {:.4} :avg-latency-ms {:.2} :total-cost-usd {:.8} :history {})",
                from, to, e.weight, e.min_harmony, e.uses, sr, avg_latency, e.total_cost_usd, hist
            )
        })
        .collect();
    edge_entries.sort();

    clear_error();
    to_c_string(format!(
        "(:epoch {} :revision {} :event-count {} :nodes ({}) :edges ({}))",
        st.epoch,
        st.revision,
        st.events.len(),
        node_entries.join(" "),
        edge_entries.join(" ")
    ))
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "harmonic matrix error lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
