//! Public API for observability — called by IPC dispatch and actors.
//!
//! Design principles:
//! - Never panic, never block the caller
//! - No-op when disabled — zero cost, safe to sprinkle everywhere
//! - Trace-level gating happens HERE, not at call sites
//! - Handle map is lazy-initialized, no setup ceremony

use std::collections::HashMap;
use std::sync::Mutex;

use crate::config::ObservabilityConfig;
use crate::context;
use crate::model::{
    dotted_order_child, dotted_order_for, new_uuid, now_iso, ObservabilityState, TraceEvent,
    TraceLevel, TraceMessage, TraceSpan,
};
use crate::sender;

use serde_json::json;

static STATE: Mutex<Option<ObservabilityState>> = Mutex::new(None);
static HANDLE_MAP: Mutex<Option<HashMap<i64, String>>> = Mutex::new(None);

// ─── Lifecycle ───────────────────────────────────────────────────────

/// Initialize observability. Returns 0 on success. Idempotent.
pub fn harmonia_observability_init() -> i32 {
    let config = ObservabilityConfig::load();
    if !config.enabled || config.api_key.is_empty() {
        eprintln!(
            "[INFO] [observability] Disabled (enabled={}, api_key_set={})",
            config.enabled,
            !config.api_key.is_empty()
        );
        let mut guard = STATE.lock().unwrap();
        *guard = Some(ObservabilityState {
            enabled: false,
            ..Default::default()
        });
        return 0;
    }

    sender::start(&config.api_url, &config.api_key, &config.project_name);

    let mut guard = STATE.lock().unwrap();
    *guard = Some(ObservabilityState {
        enabled: true,
        initialized: true,
        trace_level: config.trace_level,
        sample_rate: config.sample_rate,
        project_name: config.project_name,
        api_url: config.api_url,
        api_key: config.api_key,
        next_handle: 1,
    });

    eprintln!(
        "[INFO] [observability] Initialized (level={}, sample_rate={})",
        guard.as_ref().unwrap().trace_level.as_str(),
        guard.as_ref().unwrap().sample_rate
    );
    0
}

pub fn harmonia_observability_flush() {
    sender::flush();
}

pub fn harmonia_observability_shutdown() {
    sender::shutdown();
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.enabled = false;
            state.initialized = false;
        }
    }
}

// ─── Level checks (call before building expensive metadata) ─────────

/// True if tracing is active at any level.
pub fn harmonia_observability_enabled() -> bool {
    STATE
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.enabled && s.initialized))
        .unwrap_or(false)
}

/// True if trace level >= Standard (standard + verbose).
pub fn harmonia_observability_is_standard() -> bool {
    STATE
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref().map(|s| {
                s.enabled
                    && s.initialized
                    && matches!(s.trace_level, TraceLevel::Standard | TraceLevel::Verbose)
            })
        })
        .unwrap_or(false)
}

/// True if trace level == Verbose (everything).
pub fn harmonia_observability_is_verbose() -> bool {
    STATE
        .lock()
        .ok()
        .and_then(|g| {
            g.as_ref()
                .map(|s| s.enabled && s.initialized && s.trace_level == TraceLevel::Verbose)
        })
        .unwrap_or(false)
}

// ─── Span lifecycle ─────────────────────────────────────────────────

/// Start a new trace span. Returns handle (>0) or 0 if disabled/sampled-out.
pub fn harmonia_observability_trace_start(
    name: &str,
    kind: &str,
    parent_id: i64,
    metadata_json: &str,
) -> i64 {
    let mut guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let state = match guard.as_mut() {
        Some(s) if s.enabled && s.initialized => s,
        _ => return 0,
    };

    let handle = state.alloc_handle();
    if handle == 0 {
        return 0;
    }

    let run_id = new_uuid();
    let now = now_iso();
    let project_name = state.project_name.clone();
    drop(guard); // Release lock before context operations

    let ctx = context::current();
    let (trace_id, parent_run_id, dotted_order) = if parent_id > 0 {
        let tid = ctx.trace_id.unwrap_or_else(|| run_id.clone());
        let parent_rid = ctx.parent_run_id.clone();
        let dotted = ctx
            .dotted_order
            .as_ref()
            .map(|d| dotted_order_child(d, &run_id))
            .unwrap_or_else(|| dotted_order_for(&run_id));
        (tid, parent_rid, dotted)
    } else {
        let dotted = dotted_order_for(&run_id);
        (run_id.clone(), None, dotted)
    };

    // Set thread-local context so children inherit this span
    context::set(context::TraceContext {
        trace_id: Some(trace_id.clone()),
        parent_run_id: Some(run_id.clone()),
        dotted_order: Some(dotted_order.clone()),
    });

    let inputs = serde_json::from_str(metadata_json).unwrap_or(json!({}));

    sender::send(TraceMessage::StartRun(TraceSpan {
        run_id: run_id.clone(),
        parent_run_id,
        trace_id,
        dotted_order,
        name: name.to_string(),
        run_type: kind.to_string(),
        start_time: now,
        end_time: None,
        status: None,
        inputs,
        outputs: None,
        extra: json!({}),
        project_name,
    }));

    handle_map_insert(handle, run_id);
    handle
}

/// End a trace span. handle=0 is a no-op.
pub fn harmonia_observability_trace_end(handle: i64, status: &str, output_json: &str) {
    if handle == 0 {
        return;
    }
    if let Some(run_id) = handle_map_remove(handle) {
        let outputs = serde_json::from_str(output_json).unwrap_or(json!({}));
        sender::send(TraceMessage::EndRun {
            run_id,
            status: status.to_string(),
            outputs,
            end_time: now_iso(),
        });
    }
}

/// Fire-and-forget trace event. No-op when disabled.
pub fn harmonia_observability_trace_event(name: &str, kind: &str, metadata_json: &str) {
    let guard = match STATE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let state = match guard.as_ref() {
        Some(s) if s.enabled && s.initialized => s,
        _ => return,
    };

    let project_name = state.project_name.clone();
    drop(guard);

    let ctx = context::current();
    let metadata = serde_json::from_str(metadata_json).unwrap_or(json!({}));

    sender::send(TraceMessage::Event(TraceEvent {
        name: name.to_string(),
        run_type: kind.to_string(),
        metadata,
        project_name,
        trace_id: ctx.trace_id,
        parent_run_id: ctx.parent_run_id,
        dotted_order: ctx.dotted_order,
    }));
}

// ─── Handle map (lazy-init, lock-free happy path check) ─────────────

fn handle_map_insert(handle: i64, run_id: String) {
    if let Ok(mut guard) = HANDLE_MAP.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(handle, run_id);
    }
}

fn handle_map_remove(handle: i64) -> Option<String> {
    HANDLE_MAP
        .lock()
        .ok()
        .and_then(|mut g| g.as_mut().and_then(|m| m.remove(&handle)))
}
