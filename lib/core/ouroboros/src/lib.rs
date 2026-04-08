//! Ouroboros — self-healing crash ledger and patch writing.
//!
//! Pure Rust actor state. No FFI, no unsafe, no singletons.
//! All state owned by OuroborosState, dispatched through ComponentDescriptor.
//!
//! Crash ledger: dual-tier (recovery.log file + SQLite chronicle).
//! Patch writing: writes diff files to patch directory for evolution.

mod ledger;
mod patch;

pub use ledger::CrashEntry;

/// Actor-owned state. No globals — passed through ComponentDescriptor.
pub struct OuroborosState {
    pub(crate) recovery_log_path: String,
    pub(crate) patch_dir: String,
}

impl OuroborosState {
    /// Test constructor with isolated paths to avoid parallel test collisions.
    pub fn with_paths(recovery_log_path: String, patch_dir: String) -> Self {
        Self { recovery_log_path, patch_dir }
    }
}

impl OuroborosState {
    pub fn new() -> Self {
        let state_root = harmonia_config_store::get_config_or(
            "ouroboros-core", "global", "state-root",
            &std::env::temp_dir().join("harmonia").to_string_lossy(),
        ).unwrap_or_else(|_| std::env::temp_dir().join("harmonia").to_string_lossy().into());

        let recovery_log_path = harmonia_config_store::get_config(
            "ouroboros-core", "global", "recovery-log",
        ).ok().flatten().unwrap_or_else(|| format!("{}/recovery.log", state_root));

        let patch_dir = harmonia_config_store::get_own("ouroboros-core", "patch-dir")
            .ok().flatten()
            .unwrap_or_else(|| format!("{}/patches", state_root));

        Self { recovery_log_path, patch_dir }
    }
}

/// Dispatch IPC command. Pure: sexp in → sexp out. Actor-owned state.
pub fn dispatch(state: &mut OuroborosState, sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    let esc = harmonia_actor_protocol::sexp_escape;
    match op.as_str() {
        "healthcheck" => "(:ok :status \"ok\" :role \"repair-engine\")".into(),
        "record-crash" => {
            let component = harmonia_actor_protocol::extract_sexp_string(sexp, ":component-name")
                .or_else(|| harmonia_actor_protocol::extract_sexp_string(sexp, ":component"))
                .unwrap_or_else(|| "unknown".into());
            let detail = harmonia_actor_protocol::extract_sexp_string(sexp, ":detail")
                .unwrap_or_default();
            match ledger::record_crash(state, &component, &detail) {
                Ok(()) => "(:ok)".into(),
                Err(e) => format!("(:error \"record-crash: {}\")", esc(&e)),
            }
        }
        "last-crash" => match ledger::last_crash(state) {
            Ok(entry) => format!("(:ok :result \"{}\")", esc(&entry.to_string())),
            Err(e) => format!("(:error \"last-crash: {}\")", esc(&e)),
        },
        "history" => {
            let limit = harmonia_actor_protocol::extract_sexp_string(sexp, ":limit")
                .and_then(|s| s.parse::<usize>().ok()).unwrap_or(20);
            match ledger::history(state, limit) {
                Ok(entries) => {
                    let items = entries.iter()
                        .map(|e| format!("\"{}\"", esc(&e.to_string())))
                        .collect::<Vec<_>>().join(" ");
                    format!("(:ok :result ({}))", items)
                }
                Err(e) => format!("(:error \"history: {}\")", esc(&e)),
            }
        }
        "write-patch" => {
            let component = harmonia_actor_protocol::extract_sexp_string(sexp, ":component-name")
                .or_else(|| harmonia_actor_protocol::extract_sexp_string(sexp, ":component"))
                .unwrap_or_else(|| "unknown".into());
            let body = harmonia_actor_protocol::extract_sexp_string(sexp, ":patch-body")
                .unwrap_or_default();
            if body.is_empty() {
                return "(:error \"write-patch: :patch-body required\")".into();
            }
            match patch::write_patch(state, &component, &body) {
                Ok(path) => format!("(:ok :path \"{}\")", esc(&path)),
                Err(e) => format!("(:error \"write-patch: {}\")", esc(&e)),
            }
        }
        _ => format!("(:error \"ouroboros: unknown op '{}'\")", esc(&op)),
    }
}
