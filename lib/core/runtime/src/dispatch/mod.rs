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

/// Escape a string for embedding in an sexp response. Shared by all dispatchers.
pub(crate) fn esc(s: &str) -> String {
    harmonia_actor_protocol::sexp_escape(s)
}

/// Format a list of strings as sexp: ("a" "b" "c")
pub(crate) fn sexp_string_list(items: &[String]) -> String {
    items.iter()
        .map(|s| format!("\"{}\"", harmonia_actor_protocol::sexp_escape(s)))
        .collect::<Vec<_>>()
        .join(" ")
}

// Re-export the functions that actors.rs, ipc.rs, and supervisor.rs call
pub use matrix::dispatch_matrix_via_actor;
pub use observability::dispatch_obs_trace;

/// Dispatch vault commands (requires actor-owned VaultState).
pub(crate) fn dispatch_vault(
    sexp: &str,
    state: &mut harmonia_vault::VaultState,
) -> String {
    vault::dispatch_with_state(sexp, state)
}

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

/// Dispatch a command to the named component (synchronous, for non-matrix components).
/// Returns an sexp reply string.
pub fn dispatch(component: &str, sexp: &str) -> String {
    match component {
        "vault" => {
            // Vault requires actor-owned state. Non-actor callers should not reach here.
            format!(
                "(:error \"component 'vault' requires actor-owned state\")",
            )
        }
        "config" => config::dispatch(sexp),
        "chronicle" => chronicle::dispatch(sexp),
        "gateway" => gateway::dispatch(sexp),
        "tailnet" => tailnet::dispatch(sexp),
        "harmonic-matrix" | "matrix" => {
            format!(
                "(:error \"component 'harmonic-matrix' requires actor-owned state\")",
            )
        }
        "provider-router" => provider_router::dispatch(sexp),
        "parallel" => parallel::dispatch(sexp),
        "workspace" => workspace::dispatch(sexp),
        "observability" => observability::dispatch(sexp),
        "signalograd" | "memory-field" | "mempalace" | "terraphon" | "sessions"
        | "mcp" | "router" => {
            format!(
                "(:error \"component '{}' requires actor-owned state — route through actor\")",
                harmonia_actor_protocol::sexp_escape(component)
            )
        }
        _ => format!(
            "(:error \"unknown component: {}\")",
            harmonia_actor_protocol::sexp_escape(component)
        ),
    }
}
