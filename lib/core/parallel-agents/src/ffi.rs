use std::os::raw::c_char;

use crate::engine;
use crate::model::{clear_error, cstr_to_string, last_error_message, set_error, to_c_string};

const VERSION: &[u8] = b"harmonia-parallel-agents/0.2.0\0";

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_healthcheck() -> i32 {
    engine::healthcheck()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_init() -> i32 {
    engine::init_ffi()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_set_model_price(
    model: *const c_char,
    usd_per_1k_input: f64,
    usd_per_1k_output: f64,
) -> i32 {
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::set_model_price(&model, usd_per_1k_input, usd_per_1k_output) {
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
pub extern "C" fn harmonia_parallel_agents_submit(
    prompt: *const c_char,
    model: *const c_char,
) -> i64 {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match engine::submit(&prompt, &model) {
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

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_run_pending(max_parallel: i32) -> i32 {
    match engine::run_pending(max_parallel) {
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
pub extern "C" fn harmonia_parallel_agents_task_result(task_id: i64) -> *mut c_char {
    match engine::task_result(task_id) {
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
pub extern "C" fn harmonia_parallel_agents_report() -> *mut c_char {
    match engine::report() {
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
pub extern "C" fn harmonia_parallel_agents_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(std::ffi::CString::from_raw(ptr));
    }
}

// ===========================================================================
// Tmux CLI Agent FFI — distinct operational tier from OpenRouter
// ===========================================================================

/// Spawn a tmux CLI agent.
/// cli_type: "claude-code", "codex"
/// workdir: directory the agent operates in
/// prompt: initial prompt to send (empty string = no initial prompt)
/// Returns agent ID (>= 0) or -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_tmux_spawn(
    cli_type: *const c_char,
    workdir: *const c_char,
    prompt: *const c_char,
) -> i64 {
    let cli_type = match cstr_to_string(cli_type) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let workdir = match cstr_to_string(workdir) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::tmux_spawn(&cli_type, &workdir, &prompt) {
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

/// Spawn a custom CLI agent with explicit command and args.
#[no_mangle]
pub extern "C" fn harmonia_tmux_spawn_custom(
    command: *const c_char,
    args: *const c_char,
    workdir: *const c_char,
    prompt: *const c_char,
) -> i64 {
    let command = match cstr_to_string(command) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let args = match cstr_to_string(args) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let workdir = match cstr_to_string(workdir) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::tmux_spawn_custom(&command, &args, &workdir, &prompt) {
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

/// Poll a tmux agent's state. Returns s-expression string or null on error.
#[no_mangle]
pub extern "C" fn harmonia_tmux_poll(id: i64) -> *mut c_char {
    match engine::tmux_poll(id) {
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

/// Send text input to a tmux agent (types text + Enter). Returns 0 or -1.
#[no_mangle]
pub extern "C" fn harmonia_tmux_send(id: i64, input: *const c_char) -> i32 {
    let input = match cstr_to_string(input) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::tmux_send(id, &input) {
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

/// Send a special key (Enter, Tab, Escape, Up, Down, C-c, etc.).
#[no_mangle]
pub extern "C" fn harmonia_tmux_send_key(id: i64, key: *const c_char) -> i32 {
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match engine::tmux_send_key(id, &key) {
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

/// Approve a permission prompt on a tmux agent.
#[no_mangle]
pub extern "C" fn harmonia_tmux_approve(id: i64) -> i32 {
    match engine::tmux_approve(id) {
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

/// Deny a permission prompt on a tmux agent.
#[no_mangle]
pub extern "C" fn harmonia_tmux_deny(id: i64) -> i32 {
    match engine::tmux_deny(id) {
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

/// Confirm yes on a confirmation prompt.
#[no_mangle]
pub extern "C" fn harmonia_tmux_confirm_yes(id: i64) -> i32 {
    match engine::tmux_confirm_yes(id) {
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

/// Confirm no on a confirmation prompt.
#[no_mangle]
pub extern "C" fn harmonia_tmux_confirm_no(id: i64) -> i32 {
    match engine::tmux_confirm_no(id) {
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

/// Select an option by index (0-based) from a selection menu.
#[no_mangle]
pub extern "C" fn harmonia_tmux_select(id: i64, index: i32) -> i32 {
    match engine::tmux_select(id, index) {
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

/// Capture the terminal output of a tmux agent. Returns string or null.
#[no_mangle]
pub extern "C" fn harmonia_tmux_capture(id: i64, history_lines: i32) -> *mut c_char {
    match engine::tmux_capture(id, history_lines) {
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

/// Get full status of a specific tmux agent as s-expression.
#[no_mangle]
pub extern "C" fn harmonia_tmux_status(id: i64) -> *mut c_char {
    match engine::tmux_status(id) {
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

/// Kill a tmux agent, destroying its session.
#[no_mangle]
pub extern "C" fn harmonia_tmux_kill(id: i64) -> i32 {
    match engine::tmux_kill(id) {
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

/// Interrupt a tmux agent (send Ctrl+C).
#[no_mangle]
pub extern "C" fn harmonia_tmux_interrupt(id: i64) -> i32 {
    match engine::tmux_interrupt(id) {
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

/// List all tmux agents as s-expression.
#[no_mangle]
pub extern "C" fn harmonia_tmux_list() -> *mut c_char {
    match engine::tmux_list() {
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

/// Poll ALL active tmux agents — the swarm heartbeat.
/// Returns s-expression with collective state and per-agent status.
#[no_mangle]
pub extern "C" fn harmonia_tmux_swarm_poll() -> *mut c_char {
    match engine::tmux_swarm_poll() {
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

// ===========================================================================
// Metrics query FFI — curated data for harmonic orchestration decisions
// ===========================================================================

/// Query performance stats for a specific model.
/// Returns s-expression: (:model "x" :count N :success-rate F :avg-latency-ms F ...)
#[no_mangle]
pub extern "C" fn harmonia_metrics_model_stats(model: *const c_char) -> *mut c_char {
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    to_c_string(harmonia_provider_protocol::query_model_stats(&model))
}

/// Query best-performing models for a backend, ranked by success rate and speed.
/// Returns s-expression list of top candidates.
#[no_mangle]
pub extern "C" fn harmonia_metrics_best_models(
    backend: *const c_char,
    limit: i32,
) -> *mut c_char {
    let backend = match cstr_to_string(backend) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    to_c_string(harmonia_provider_protocol::query_best_models_for_task(
        &backend,
        if limit <= 0 { 5 } else { limit },
    ))
}

/// Full LLM backend performance report from metrics database.
#[no_mangle]
pub extern "C" fn harmonia_metrics_llm_report() -> *mut c_char {
    to_c_string(harmonia_provider_protocol::query_llm_report())
}

/// Tmux agent event summary from metrics database.
#[no_mangle]
pub extern "C" fn harmonia_metrics_tmux_report() -> *mut c_char {
    to_c_string(harmonia_provider_protocol::query_tmux_report())
}

/// Combined telemetry digest for harmonic orchestration decisions.
/// Returns compact s-expression with the most actionable data across all tiers.
#[no_mangle]
pub extern "C" fn harmonia_metrics_telemetry_digest() -> *mut c_char {
    to_c_string(harmonia_provider_protocol::query_telemetry_digest())
}

/// Sync model catalogue from OpenRouter API.
/// Fetches all 290+ models with pricing into the models table.
/// api_key: OpenRouter API key. Returns count of models synced or -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_metrics_sync_models(api_key: *const c_char) -> i64 {
    let key = match cstr_to_string(api_key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match harmonia_provider_protocol::sync_models_from_openrouter(&key) {
        Ok(count) => {
            clear_error();
            count as i64
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

/// Run an arbitrary SELECT SQL query against the metrics database.
/// Returns JSON array of result rows. Only SELECT/WITH/EXPLAIN allowed.
/// Caller must free with harmonia_parallel_agents_free_string.
#[no_mangle]
pub extern "C" fn harmonia_metrics_query_json(sql: *const c_char) -> *mut c_char {
    let sql = match cstr_to_string(sql) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    match harmonia_provider_protocol::query_sql(&sql) {
        Ok(json) => {
            clear_error();
            to_c_string(json)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

/// Run an arbitrary SELECT SQL query, return results as s-expression.
/// Each row becomes a plist: (:col-name val ...).
/// Caller must free with harmonia_parallel_agents_free_string.
#[no_mangle]
pub extern "C" fn harmonia_metrics_query_sexp(sql: *const c_char) -> *mut c_char {
    let sql = match cstr_to_string(sql) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    match harmonia_provider_protocol::query_sql_sexp(&sql) {
        Ok(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

// ===========================================================================
// Metrics → Harmonic Matrix Bridge FFI
// ===========================================================================

/// Query recent LLM perf data as s-expression route entries for harmonic matrix.
/// The Lisp conductor iterates and feeds each into `harmonic_matrix_observe_route()`.
/// since_ts: Unix timestamp — only include data newer than this.
/// Caller must free with harmonia_parallel_agents_free_string.
#[no_mangle]
pub extern "C" fn harmonia_metrics_bridge_routes(since_ts: i64) -> *mut c_char {
    match harmonia_provider_protocol::bridge_perf_to_routes(since_ts) {
        Ok(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_parallel_agents_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_non_null() {
        assert!(!harmonia_parallel_agents_version().is_null());
    }
}
