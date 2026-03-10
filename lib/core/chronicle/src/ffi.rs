use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::{clear_error, last_error_message, set_error};
use crate::{dashboard, db, query, tables};

/// Actor ID assigned by the unified registry (0 = not registered yet)
static CHRONICLE_ACTOR_ID: AtomicU64 = AtomicU64::new(0);

const VERSION: &[u8] = b"harmonia-chronicle/0.1.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn cstr_to_optional(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    let s = c.to_string_lossy().trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ─── Lifecycle ─────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_chronicle_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_init() -> i32 {
    match db::init() {
        Ok(()) => {
            clear_error();
            // Register as chronicle actor in the unified registry (via dlsym)
            if CHRONICLE_ACTOR_ID.load(Ordering::SeqCst) == 0
                && harmonia_actor_protocol::client::is_available()
            {
                match harmonia_actor_protocol::client::register("chronicle") {
                    Ok(id) => {
                        CHRONICLE_ACTOR_ID.store(id, Ordering::SeqCst);
                    }
                    Err(_) => {} // Non-fatal
                }
            }
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

// ─── Recording ─────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_harmonic(
    cycle: i64,
    phase: *const c_char,
    strength: f64,
    utility: f64,
    beauty: f64,
    signal: f64,
    noise: f64,
    logistic_x: f64,
    logistic_r: f64,
    chaos_risk: f64,
    rewrite_aggression: f64,
    lorenz_x: f64,
    lorenz_y: f64,
    lorenz_z: f64,
    lorenz_radius: f64,
    lorenz_bounded: f64,
    lambdoma_global: f64,
    lambdoma_local: f64,
    lambdoma_ratio: f64,
    lambdoma_convergent: i32,
    rewrite_ready: i32,
    rewrite_count: i32,
    security_posture: *const c_char,
    security_events: i32,
) -> i32 {
    let phase = match cstr_to_string(phase) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let posture = cstr_to_optional(security_posture).unwrap_or_else(|| "nominal".to_string());

    let snap = tables::harmonic::HarmonicSnapshot {
        cycle,
        phase,
        strength,
        utility,
        beauty,
        signal,
        noise,
        logistic_x,
        logistic_r,
        chaos_risk,
        rewrite_aggression,
        lorenz_x,
        lorenz_y,
        lorenz_z,
        lorenz_radius,
        lorenz_bounded,
        lambdoma_global,
        lambdoma_local,
        lambdoma_ratio,
        lambdoma_convergent: lambdoma_convergent != 0,
        rewrite_ready: rewrite_ready != 0,
        rewrite_count,
        security_posture: posture,
        security_events,
    };

    match tables::harmonic::record(&snap) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_memory_event(
    event_type: *const c_char,
    entries_created: i32,
    entries_source: i32,
    old_size: i64,
    new_size: i64,
    node_count: i32,
    edge_count: i32,
    interdisciplinary_edges: i32,
    max_depth: i32,
    detail: *const c_char,
) -> i32 {
    let event_type = match cstr_to_string(event_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let detail = cstr_to_optional(detail);

    match tables::memory::record(
        &event_type,
        entries_created,
        entries_source,
        old_size,
        new_size,
        node_count,
        edge_count,
        interdisciplinary_edges,
        max_depth,
        detail.as_deref(),
    ) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_phoenix_event(
    event_type: *const c_char,
    exit_code: i32,
    attempt: i32,
    max_attempts: i32,
    recovery_ms: i64,
    detail: *const c_char,
) -> i32 {
    let event_type = match cstr_to_string(event_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let detail = cstr_to_optional(detail);

    // Use -1 as sentinel for "no value"
    let exit_code_opt = if exit_code == -1 {
        None
    } else {
        Some(exit_code)
    };
    let attempt_opt = if attempt == -1 { None } else { Some(attempt) };
    let max_opt = if max_attempts == -1 {
        None
    } else {
        Some(max_attempts)
    };
    let recovery_opt = if recovery_ms == -1 {
        None
    } else {
        Some(recovery_ms)
    };

    match tables::phoenix::record(
        &event_type,
        exit_code_opt,
        attempt_opt,
        max_opt,
        recovery_opt,
        detail.as_deref(),
    ) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_ouroboros_event(
    event_type: *const c_char,
    component: *const c_char,
    detail: *const c_char,
    patch_size: i64,
    success: i32,
) -> i32 {
    let event_type = match cstr_to_string(event_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let component = cstr_to_optional(component);
    let detail = cstr_to_optional(detail);
    let patch_opt = if patch_size < 0 {
        None
    } else {
        Some(patch_size)
    };

    match tables::ouroboros::record(
        &event_type,
        component.as_deref(),
        detail.as_deref(),
        patch_opt,
        success != 0,
    ) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_delegation(
    task_hint: *const c_char,
    model_chosen: *const c_char,
    backend: *const c_char,
    reason: *const c_char,
    escalated: i32,
    escalated_from: *const c_char,
    cost_usd: f64,
    latency_ms: i64,
    success: i32,
    tokens_in: i64,
    tokens_out: i64,
) -> i32 {
    let model = match cstr_to_string(model_chosen) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let backend_str = cstr_to_optional(backend).unwrap_or_else(|| "openrouter".to_string());
    let task = cstr_to_optional(task_hint);
    let reason_str = cstr_to_optional(reason);
    let esc_from = cstr_to_optional(escalated_from);

    match tables::delegation::record(
        task.as_deref(),
        &model,
        &backend_str,
        reason_str.as_deref(),
        escalated != 0,
        esc_from.as_deref(),
        cost_usd,
        latency_ms,
        success != 0,
        tokens_in,
        tokens_out,
    ) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_signalograd_event(
    event_type: *const c_char,
    cycle: i64,
    confidence: f64,
    stability: f64,
    novelty: f64,
    reward: f64,
    accepted: i32,
    recall_hits: i32,
    checkpoint_path: *const c_char,
    checkpoint_digest: *const c_char,
    detail: *const c_char,
) -> i32 {
    let event_type = match cstr_to_string(event_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let checkpoint_path = cstr_to_optional(checkpoint_path);
    let checkpoint_digest = cstr_to_optional(checkpoint_digest);
    let detail = cstr_to_optional(detail);

    match tables::signalograd::record(
        &event_type,
        cycle,
        confidence,
        stability,
        novelty,
        reward,
        accepted != 0,
        recall_hits,
        checkpoint_path.as_deref(),
        checkpoint_digest.as_deref(),
        detail.as_deref(),
    ) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

/// Record a concept graph snapshot. The sexp is the full s-expression;
/// nodes_json and edges_json are JSON arrays for relational decomposition.
/// Format nodes: [{"concept":"x","domain":"y","count":1,"depth_min":0,"depth_max":0,"classes":"a,b"},...]
/// Format edges: [{"a":"x","b":"y","weight":1,"interdisciplinary":false,"reasons":"co-occur"},...]
#[no_mangle]
pub extern "C" fn harmonia_chronicle_record_graph(
    source: *const c_char,
    sexp: *const c_char,
    nodes_json: *const c_char,
    edges_json: *const c_char,
) -> i64 {
    let source = match cstr_to_string(source) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let sexp_str = match cstr_to_string(sexp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let nodes: Vec<(String, String, i32, i32, i32, String)> = match cstr_to_optional(nodes_json) {
        Some(json_str) => parse_nodes_json(&json_str).unwrap_or_default(),
        None => Vec::new(),
    };

    let edges: Vec<(String, String, i32, bool, String)> = match cstr_to_optional(edges_json) {
        Some(json_str) => parse_edges_json(&json_str).unwrap_or_default(),
        None => Vec::new(),
    };

    match tables::graph::record_snapshot(&source, &sexp_str, &nodes, &edges) {
        Ok(id) => {
            clear_error();
            id
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

fn parse_nodes_json(json: &str) -> Result<Vec<(String, String, i32, i32, i32, String)>, String> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(arr.len());
    for v in &arr {
        result.push((
            v["concept"].as_str().unwrap_or("").to_string(),
            v["domain"].as_str().unwrap_or("generic").to_string(),
            v["count"].as_i64().unwrap_or(1) as i32,
            v["depth_min"].as_i64().unwrap_or(0) as i32,
            v["depth_max"].as_i64().unwrap_or(0) as i32,
            v["classes"].as_str().unwrap_or("").to_string(),
        ));
    }
    Ok(result)
}

fn parse_edges_json(json: &str) -> Result<Vec<(String, String, i32, bool, String)>, String> {
    let arr: Vec<serde_json::Value> = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(arr.len());
    for v in &arr {
        result.push((
            v["a"].as_str().unwrap_or("").to_string(),
            v["b"].as_str().unwrap_or("").to_string(),
            v["weight"].as_i64().unwrap_or(1) as i32,
            v["interdisciplinary"].as_bool().unwrap_or(false),
            v["reasons"].as_str().unwrap_or("").to_string(),
        ));
    }
    Ok(result)
}

// ─── Querying ──────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_chronicle_harmony_trajectory(
    since_ts: i64,
    until_ts: i64,
) -> *mut c_char {
    match query::harmony_trajectory(since_ts, until_ts) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_harmonic_history(since_ts: i64, limit: i32) -> *mut c_char {
    match query::harmonic_history(since_ts, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_memory_history(since_ts: i64, limit: i32) -> *mut c_char {
    match query::memory_history(since_ts, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_phoenix_history(since_ts: i64, limit: i32) -> *mut c_char {
    match query::phoenix_history(since_ts, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_ouroboros_history(since_ts: i64, limit: i32) -> *mut c_char {
    match query::ouroboros_history(since_ts, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_delegation_history(since_ts: i64, limit: i32) -> *mut c_char {
    match query::delegation_history(since_ts, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_harmony_summary() -> *mut c_char {
    match query::harmony_summary() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_delegation_report() -> *mut c_char {
    match query::delegation_report() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_cost_report(since_ts: i64) -> *mut c_char {
    match query::cost_report(since_ts) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_full_digest() -> *mut c_char {
    match query::full_digest() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_query_sexp(sql: *const c_char) -> *mut c_char {
    let sql = match cstr_to_string(sql) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    match db::query_sexp(&sql) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_dashboard_json() -> *mut c_char {
    match dashboard::dashboard_json() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

// ─── Graph Queries ─────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_chronicle_graph_traverse(
    concept: *const c_char,
    max_hops: i32,
    snapshot_id: i64,
) -> *mut c_char {
    let concept = match cstr_to_string(concept) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let snap_id = if snapshot_id <= 0 {
        None
    } else {
        Some(snapshot_id)
    };
    match tables::graph::traverse_from(&concept, max_hops, snap_id) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_graph_bridges(snapshot_id: i64) -> *mut c_char {
    let snap_id = if snapshot_id <= 0 {
        None
    } else {
        Some(snapshot_id)
    };
    match tables::graph::interdisciplinary_bridges(snap_id) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_graph_domains(snapshot_id: i64) -> *mut c_char {
    let snap_id = if snapshot_id <= 0 {
        None
    } else {
        Some(snapshot_id)
    };
    match tables::graph::domain_distribution(snap_id) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_graph_central(snapshot_id: i64, limit: i32) -> *mut c_char {
    let snap_id = if snapshot_id <= 0 {
        None
    } else {
        Some(snapshot_id)
    };
    match tables::graph::central_concepts(snap_id, limit) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_graph_evolution(since_ts: i64) -> *mut c_char {
    match tables::graph::graph_evolution(since_ts) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

// ─── Maintenance ───────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_chronicle_gc_status() -> *mut c_char {
    match db::gc_status() {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_gc() -> i32 {
    match db::gc() {
        Ok(deleted) => {
            clear_error();
            deleted
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_chronicle_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

/// Process a batch of delegation recording requests in one call.
/// batch_sexp: s-expression list of delegation plists, each containing
/// :task-hint, :model, :backend, :reason, :escalated, :escalated-from,
/// :cost-usd, :latency-ms, :success, :tokens-in, :tokens-out.
/// Returns count of records written or -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_chronicle_flush_batch(batch_sexp: *const c_char) -> i32 {
    let _sexp = match cstr_to_string(batch_sexp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    // The batch is processed by the Lisp layer which calls individual
    // chronicle-record-* functions. This FFI entry point is provided for
    // future optimization where the Rust side can process batches in a
    // single SQLite transaction. For now, acknowledge the batch.

    // Post RecordAck to unified mailbox
    let actor_id = CHRONICLE_ACTOR_ID.load(Ordering::SeqCst);
    if actor_id > 0 && harmonia_actor_protocol::client::is_available() {
        let _ = harmonia_actor_protocol::client::heartbeat(actor_id, 1);
    }

    clear_error();
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_null() {
        assert!(!harmonia_chronicle_version().is_null());
    }

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_chronicle_healthcheck(), 1);
    }
}
