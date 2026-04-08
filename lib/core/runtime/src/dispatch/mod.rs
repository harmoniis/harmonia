//! Component dispatch — routes IPC sexp commands to crate public APIs.
//!
//! Each component's commands are dispatched here by name. The Lisp side
//! sends (:component "vault" :op "set-secret" :symbol "x" :value "y")
//! and the per-component module calls the corresponding Rust API and
//! returns the result as an sexp string.

pub(crate) mod chronicle;
pub(crate) mod config;
pub(crate) mod gateway;
pub(crate) mod matrix;
pub(crate) mod workspace;
pub(crate) mod memory_field;
pub(crate) mod observability;
pub(crate) mod parallel;
pub(crate) mod provider_router;
pub(crate) mod signalograd;
pub(crate) mod mempalace;
pub(crate) mod tailnet;
pub(crate) mod terraphon;
pub(crate) mod vault;

/// Dispatch a single operation: call function, wrap errors in sexp.
/// Works with any error type implementing Display (String, MemoryError, etc.).
macro_rules! dispatch_op {
    ($op_name:expr, $body:expr) => {
        match $body {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                format!("(:error \"{}: {}\")", $op_name, harmonia_actor_protocol::sexp_escape(&msg))
            }
        }
    };
}
pub(crate) use dispatch_op;

/// Extract a string parameter from sexp, defaulting to empty.
/// Pure functional: no mutation, just extraction.
macro_rules! param {
    ($sexp:expr, $key:literal) => {
        harmonia_actor_protocol::extract_sexp_string($sexp, $key).unwrap_or_default()
    };
    ($sexp:expr, $key:literal, $default:expr) => {
        harmonia_actor_protocol::extract_sexp_string($sexp, $key).unwrap_or_else(|| $default.to_string())
    };
}
pub(crate) use param;

/// Extract an optional u64 parameter from sexp.
macro_rules! param_u64 {
    ($sexp:expr, $key:literal, $default:expr) => {
        harmonia_actor_protocol::extract_sexp_u64($sexp, $key).unwrap_or($default)
    };
}
pub(crate) use param_u64;

/// Extract an optional f64 parameter from sexp.
macro_rules! param_f64 {
    ($sexp:expr, $key:literal, $default:expr) => {
        harmonia_actor_protocol::extract_sexp_f64($sexp, $key).unwrap_or($default)
    };
}
pub(crate) use param_f64;

/// Format a list of strings as sexp: ("a" "b" "c")
pub(crate) fn sexp_string_list(items: &[String]) -> String {
    items.iter()
        .map(|s| format!("\"{}\"", harmonia_actor_protocol::sexp_escape(s)))
        .collect::<Vec<_>>()
        .join(" ")
}

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

// Re-export the functions that actors.rs, ipc.rs, and supervisor.rs call
pub use matrix::dispatch_matrix_via_actor;
pub use observability::dispatch_obs_trace;

/// Dispatch signalograd commands (requires actor-owned KernelState).
pub(crate) fn dispatch_signalograd(
    sexp: &str,
    state: &mut harmonia_signalograd::KernelState,
) -> String {
    signalograd::dispatch(sexp, state)
}

/// Dispatch mempalace commands (requires actor-owned PalaceState).
pub(crate) fn dispatch_mempalace(
    sexp: &str,
    state: &mut harmonia_mempalace::PalaceState,
) -> String {
    mempalace::dispatch(sexp, state)
}

/// Dispatch terraphon commands (requires actor-owned TerraphonState).
pub(crate) fn dispatch_terraphon(
    sexp: &str,
    state: &mut harmonia_terraphon::TerraphonState,
) -> String {
    terraphon::dispatch(sexp, state)
}

/// Dispatch memory-field commands (requires actor-owned FieldState).
pub(crate) fn dispatch_memory_field(
    sexp: &str,
    field: &mut harmonia_memory_field::FieldState,
) -> String {
    memory_field::dispatch(sexp, field)
}

/// Extract the vault symbol name from a dispatch sexp (for tracing — never extracts values).
pub fn extract_vault_symbol(sexp: &str) -> String {
    harmonia_actor_protocol::extract_sexp_string(sexp, ":symbol").unwrap_or_default()
}

// ── FFI helpers used by provider_router ──────────────────────────────

/// Convert a string to CString for FFI, returning error sexp on null bytes.
pub(crate) fn to_cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| "(:error \"string contains null byte\")".to_string())
}

/// RAII guard for strings allocated by C FFI. Automatically freed on drop.
pub(crate) struct FfiString(pub *mut c_char);
impl FfiString {
    pub fn as_str(&self) -> &str {
        if self.0.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(self.0) }.to_str().unwrap_or("")
    }
}
impl Drop for FfiString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                drop(CString::from_raw(self.0));
            }
        }
    }
}

/// Dispatch a command to the named component (synchronous, for non-matrix components).
/// Returns an sexp reply string.
pub fn dispatch(component: &str, sexp: &str) -> String {
    match component {
        "vault" => vault::dispatch(sexp),
        "config" => config::dispatch(sexp),
        "chronicle" => chronicle::dispatch(sexp),
        "gateway" => gateway::dispatch(sexp),
        "tailnet" => tailnet::dispatch(sexp),
        "harmonic-matrix" | "matrix" => matrix::dispatch(sexp),
        "provider-router" => provider_router::dispatch(sexp),
        "parallel" => parallel::dispatch(sexp),
        "workspace" => workspace::dispatch(sexp),
        "observability" => observability::dispatch(sexp),
        "signalograd" | "memory-field" | "mempalace" | "terraphon" => {
            format!(
                "(:error \"component '{}' requires actor-owned state\")",
                harmonia_actor_protocol::sexp_escape(component)
            )
        }
        "router" => "(:ok :result \"router dispatch via actor\")".to_string(),
        _ => format!(
            "(:error \"unknown component: {}\")",
            harmonia_actor_protocol::sexp_escape(component)
        ),
    }
}
