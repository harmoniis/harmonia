use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub(crate) struct Task {
    pub(crate) id: u64,
    pub(crate) prompt: String,
    pub(crate) model: String,
    pub(crate) status: String,
    pub(crate) response: String,
    pub(crate) error: String,
    pub(crate) latency_ms: u64,
    pub(crate) cost_usd: f64,
    pub(crate) success: bool,
    pub(crate) verified: bool,
    pub(crate) verification_source: String,
    pub(crate) verification_detail: String,
    pub(crate) created_at: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ModelPrice {
    pub(crate) usd_per_1k_input: f64,
    pub(crate) usd_per_1k_output: f64,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) next_id: u64,
    pub(crate) tasks: HashMap<u64, Task>,
    pub(crate) prices: HashMap<String, ModelPrice>,
}

static STATE: OnceLock<RwLock<State>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

pub(crate) fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| {
        RwLock::new(State {
            next_id: 1,
            ..State::default()
        })
    })
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub(crate) fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

pub(crate) fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

pub(crate) fn last_error_message() -> String {
    last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "parallel error lock poisoned".to_string())
}

pub(crate) fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

pub(crate) fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn state_root() -> String {
    env::var("HARMONIA_STATE_ROOT").unwrap_or_else(|_| {
        env::temp_dir()
            .join("harmonia")
            .to_string_lossy()
            .to_string()
    })
}

pub(crate) fn metrics_log_path() -> String {
    env::var("HARMONIA_PARALLEL_METRICS_LOG")
        .unwrap_or_else(|_| format!("{}/parallel_agents_metrics.tsv", state_root()))
}

pub(crate) fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub(crate) fn append_metric_line(task: &Task) {
    let path = metrics_log_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{}\t{}\t{}\t{}\t{:.8}\t{}\t{}\t{}\t{}",
            task.created_at,
            task.id,
            task.model,
            task.latency_ms,
            task.cost_usd,
            if task.success { 1 } else { 0 },
            if task.verified { 1 } else { 0 },
            task.verification_source,
            json_escape(&task.verification_detail)
        );
    }
}
