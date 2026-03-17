//! C-ABI FFI exports for observability.
//!
//! All functions are safe to call even when tracing is disabled — they
//! become no-ops returning 0 (handle) or 0 (success).

use crate::config::ObservabilityConfig;
use crate::context::{self, TraceContext, TraceContextGuard};
use crate::model::{
    dotted_order_child, dotted_order_root, global_state, handle_map, new_uuid, now_iso,
    TraceEvent, TraceMessage, TraceSpan,
};
use crate::sender;
use std::ffi::CStr;
use std::os::raw::c_char;

// ─── Helpers ─────────────────────────────────────────────────────────

unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(ptr).to_str().unwrap_or("")
    }
}

fn parse_metadata_sexp(sexp: &str) -> serde_json::Value {
    // Simple sexp-to-json: convert (:key "val" :key2 "val2") to {"key": "val", "key2": "val2"}
    if sexp.is_empty() || sexp == "()" || sexp == "nil" {
        return serde_json::json!({});
    }

    let mut map = serde_json::Map::new();
    let trimmed = sexp.trim().trim_start_matches('(').trim_end_matches(')');
    let mut chars = trimmed.chars().peekable();
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        match c {
            '"' if !in_string => {
                in_string = true;
            }
            '"' if in_string => {
                in_string = false;
                tokens.push(current.clone());
                current.clear();
            }
            ' ' | '\t' | '\n' if !in_string => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            '\\' if in_string => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            _ => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    // Parse key-value pairs: :key value :key2 value2
    let mut i = 0;
    while i + 1 < tokens.len() {
        let key = &tokens[i];
        let val = &tokens[i + 1];
        if let Some(k) = key.strip_prefix(':') {
            map.insert(k.to_string(), serde_json::Value::String(val.clone()));
            i += 2;
        } else {
            i += 1;
        }
    }

    serde_json::Value::Object(map)
}

// ─── FFI exports ─────────────────────────────────────────────────────

/// Initialize the observability subsystem. Returns 0 on success, -1 on error.
/// `config_sexp` is currently unused (config read from config-store + vault).
#[no_mangle]
pub extern "C" fn harmonia_observability_init(_config_sexp: *const c_char) -> i32 {
    let config = ObservabilityConfig::load();

    {
        let mut st = match global_state().write() {
            Ok(st) => st,
            Err(_) => return -1,
        };

        st.enabled = config.enabled;
        st.trace_level = config.trace_level;
        st.sample_rate = config.sample_rate;
        st.project_name = config.project_name.clone();
        st.api_url = config.api_url.clone();
        st.api_key = config.api_key.clone();
        st.initialized = true;
    }

    if config.enabled && !config.api_key.is_empty() {
        sender::start(&config.api_url, &config.api_key, &config.project_name);
    }

    0
}

/// Start a new trace span. Returns a handle (>0) or 0 if disabled/sampled-out.
///
/// - `name`: span name (e.g., "orchestrate-signal")
/// - `kind`: run type — "chain", "llm", "tool", "agent"
/// - `parent_id`: parent handle from a previous trace_start, or 0 for root
/// - `metadata_sexp`: optional metadata as s-expression string
#[no_mangle]
pub extern "C" fn harmonia_observability_trace_start(
    name: *const c_char,
    kind: *const c_char,
    parent_id: i64,
    metadata_sexp: *const c_char,
) -> i64 {
    let name_str = unsafe { cstr_to_str(name) };
    let kind_str = unsafe { cstr_to_str(kind) };
    let meta_str = unsafe { cstr_to_str(metadata_sexp) };

    let (handle, project_name) = {
        let mut st = match global_state().write() {
            Ok(st) => st,
            Err(_) => return 0,
        };
        if !st.enabled || !st.initialized {
            return 0;
        }
        (st.alloc_handle(), st.project_name.clone())
    };

    if handle == 0 {
        return 0;
    }

    let run_id = new_uuid();
    let now = now_iso();
    let metadata = parse_metadata_sexp(meta_str);

    // Determine parent context
    let (parent_run_id, trace_id, dotted) = if parent_id > 0 {
        // Look up parent span
        if let Ok(map) = handle_map().lock() {
            if let Some(parent) = map.get(&parent_id) {
                let trace = parent.trace_id.clone();
                let dot = dotted_order_child(&parent.dotted_order);
                (Some(parent.run_id.clone()), trace, dot)
            } else {
                // Parent not found — check thread-local context
                let ctx = context::current();
                if let Some(tid) = ctx.trace_id {
                    let dot = ctx
                        .dotted_order
                        .map(|d| dotted_order_child(&d))
                        .unwrap_or_else(dotted_order_root);
                    (ctx.parent_run_id, tid, dot)
                } else {
                    (None, run_id.clone(), dotted_order_root())
                }
            }
        } else {
            (None, run_id.clone(), dotted_order_root())
        }
    } else {
        // Root span — check thread-local context
        let ctx = context::current();
        if let Some(tid) = ctx.trace_id {
            let dot = ctx
                .dotted_order
                .map(|d| dotted_order_child(&d))
                .unwrap_or_else(dotted_order_root);
            (ctx.parent_run_id, tid, dot)
        } else {
            (None, run_id.clone(), dotted_order_root())
        }
    };

    let span = TraceSpan {
        run_id: run_id.clone(),
        parent_run_id,
        trace_id: trace_id.clone(),
        dotted_order: dotted.clone(),
        name: name_str.to_string(),
        run_type: kind_str.to_string(),
        start_time: now,
        end_time: None,
        status: None,
        inputs: metadata,
        outputs: None,
        extra: serde_json::json!({}),
        project_name,
    };

    // Store span for later end/lookup
    if let Ok(mut map) = handle_map().lock() {
        map.insert(handle, span.clone());
    }

    // Set thread-local context for children
    let _guard = TraceContextGuard::new(TraceContext {
        trace_id: Some(trace_id),
        parent_run_id: Some(run_id),
        dotted_order: Some(dotted),
    });

    // Send to background thread
    sender::send(TraceMessage::StartRun(span));

    handle
}

/// End a trace span. Returns 0 on success.
///
/// - `trace_handle`: handle from trace_start (0 = no-op)
/// - `status`: "success" or "error"
/// - `output_sexp`: output data as s-expression string
#[no_mangle]
pub extern "C" fn harmonia_observability_trace_end(
    trace_handle: i64,
    status: *const c_char,
    output_sexp: *const c_char,
) -> i32 {
    if trace_handle == 0 {
        return 0;
    }

    let status_str = unsafe { cstr_to_str(status) };
    let output_str = unsafe { cstr_to_str(output_sexp) };
    let outputs = parse_metadata_sexp(output_str);

    let run_id = if let Ok(mut map) = handle_map().lock() {
        map.remove(&trace_handle).map(|s| s.run_id)
    } else {
        None
    };

    if let Some(run_id) = run_id {
        sender::send(TraceMessage::EndRun {
            run_id,
            status: status_str.to_string(),
            outputs,
            end_time: now_iso(),
        });
    }

    0
}

/// Fire-and-forget trace event. Returns 0 on success.
///
/// - `name`: event name
/// - `kind`: run type
/// - `metadata_sexp`: event data as s-expression string
#[no_mangle]
pub extern "C" fn harmonia_observability_trace_event(
    name: *const c_char,
    kind: *const c_char,
    metadata_sexp: *const c_char,
) -> i32 {
    let name_str = unsafe { cstr_to_str(name) };
    let kind_str = unsafe { cstr_to_str(kind) };
    let meta_str = unsafe { cstr_to_str(metadata_sexp) };

    {
        let st = match global_state().read() {
            Ok(st) => st,
            Err(_) => return 0,
        };
        if !st.enabled || !st.initialized {
            return 0;
        }
    }

    let metadata = parse_metadata_sexp(meta_str);
    let ctx = context::current();

    let project_name = global_state()
        .read()
        .map(|st| st.project_name.clone())
        .unwrap_or_else(|_| "harmonia".to_string());

    sender::send(TraceMessage::Event(TraceEvent {
        name: name_str.to_string(),
        run_type: kind_str.to_string(),
        metadata,
        project_name,
        trace_id: ctx.trace_id,
        parent_run_id: ctx.parent_run_id,
        dotted_order: ctx.dotted_order,
    }));

    0
}

/// Flush pending traces to LangSmith. Blocks briefly. Returns 0.
#[no_mangle]
pub extern "C" fn harmonia_observability_flush() -> i32 {
    sender::flush();
    0
}

/// Shut down the observability subsystem. Flushes and stops the sender thread.
#[no_mangle]
pub extern "C" fn harmonia_observability_shutdown() -> i32 {
    sender::shutdown();
    0
}
