//! Public API for observability — called by IPC dispatch and actors.
//!
//! All functions are safe to call even when tracing is disabled or not initialized.
//! They never panic or block the caller.

use std::sync::Mutex;

use crate::config::ObservabilityConfig;
use crate::context;
use crate::model::{
    dotted_order_child, dotted_order_for, new_uuid, now_iso, ObservabilityState, TraceEvent,
    TraceMessage, TraceSpan,
};
use crate::sender;

use serde_json::json;

static STATE: Mutex<Option<ObservabilityState>> = Mutex::new(None);

/// Initialize observability: load config, start background sender.
/// Returns 0 on success, -1 on failure. Safe to call multiple times.
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

    // Start background sender thread
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

/// Start a new trace span. Returns handle (>0) or 0 if disabled/sampled-out.
/// `kind` is one of: "chain", "llm", "tool", "agent"
/// `parent_id` is 0 for root spans, or the handle of the parent span.
/// `metadata_json` is a JSON string of key-value pairs for inputs.
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

    // Determine trace context: either inherit from parent or create new root
    let ctx = context::current();
    let (trace_id, parent_run_id, dotted_order) = if parent_id > 0 {
        // Child span: inherit parent's trace context
        let tid = ctx.trace_id.unwrap_or_else(|| run_id.clone());
        let parent_rid = ctx.parent_run_id.clone();
        let dotted = ctx
            .dotted_order
            .as_ref()
            .map(|d| dotted_order_child(d, &run_id))
            .unwrap_or_else(|| dotted_order_for(&run_id));
        (tid, parent_rid, dotted)
    } else {
        // Root span
        let dotted = dotted_order_for(&run_id);
        (run_id.clone(), None, dotted)
    };

    // Set thread-local context for children
    let _guard = context::TraceContextGuard::new(context::TraceContext {
        trace_id: Some(trace_id.clone()),
        parent_run_id: Some(run_id.clone()),
        dotted_order: Some(dotted_order.clone()),
    });
    // We can't hold the guard across calls, so set it directly
    context::set(context::TraceContext {
        trace_id: Some(trace_id.clone()),
        parent_run_id: Some(run_id.clone()),
        dotted_order: Some(dotted_order.clone()),
    });

    let inputs = serde_json::from_str(metadata_json).unwrap_or(json!({}));

    let span = TraceSpan {
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
    };

    sender::send(TraceMessage::StartRun(span));

    // Store run_id keyed by handle for trace_end lookup
    HANDLE_MAP
        .lock()
        .ok()
        .map(|mut m| HandleMapExt::insert(&mut *m, handle, run_id));

    handle
}

/// End a trace span. handle=0 is a no-op.
pub fn harmonia_observability_trace_end(handle: i64, status: &str, output_json: &str) {
    if handle == 0 {
        return;
    }

    let run_id = HANDLE_MAP.lock().ok().and_then(|mut m| m.remove(&handle));

    if let Some(run_id) = run_id {
        let outputs = serde_json::from_str(output_json).unwrap_or(json!({}));
        sender::send(TraceMessage::EndRun {
            run_id,
            status: status.to_string(),
            outputs,
            end_time: now_iso(),
        });
    }

    // Restore parent context (best-effort — context is thread-local)
}

/// Fire-and-forget trace event. Always succeeds (no-op when disabled).
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

    let event = TraceEvent {
        name: name.to_string(),
        run_type: kind.to_string(),
        metadata,
        project_name,
        trace_id: ctx.trace_id,
        parent_run_id: ctx.parent_run_id,
        dotted_order: ctx.dotted_order,
    };

    sender::send(TraceMessage::Event(event));
}

/// Flush pending traces.
pub fn harmonia_observability_flush() {
    sender::flush();
}

/// Shut down observability.
pub fn harmonia_observability_shutdown() {
    sender::shutdown();
    if let Ok(mut guard) = STATE.lock() {
        if let Some(state) = guard.as_mut() {
            state.enabled = false;
            state.initialized = false;
        }
    }
}

// Handle → run_id mapping for trace_end
use std::collections::HashMap;
static HANDLE_MAP: Mutex<Option<HashMap<i64, String>>> = Mutex::new(None);

trait HandleMapExt {
    fn insert(&mut self, handle: i64, run_id: String);
    fn remove(&mut self, handle: &i64) -> Option<String>;
}

impl HandleMapExt for Option<HashMap<i64, String>> {
    fn insert(&mut self, handle: i64, run_id: String) {
        self.get_or_insert_with(HashMap::new).insert(handle, run_id);
    }
    fn remove(&mut self, handle: &i64) -> Option<String> {
        self.as_mut().and_then(|m| m.remove(handle))
    }
}
