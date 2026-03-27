use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{State, StoreConfig};

// DEPRECATED: singleton globals — the runtime actor should own State directly.
// Kept for legacy FFI wrappers in ops/reports/store that still lock-and-mutate.
#[deprecated(note = "singleton elimination: runtime actor owns State directly")]
#[allow(dead_code)]
static LEGACY_STATE: OnceLock<RwLock<State>> = OnceLock::new();
#[deprecated(note = "singleton elimination: runtime actor owns error slot directly")]
#[allow(dead_code)]
static LEGACY_LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
#[deprecated(note = "singleton elimination: runtime actor owns StoreConfig directly")]
#[allow(dead_code)]
static LEGACY_STORE_CONFIG: OnceLock<RwLock<StoreConfig>> = OnceLock::new();

#[deprecated(note = "singleton elimination: use actor-owned State")]
#[allow(dead_code, deprecated)]
pub fn state() -> &'static RwLock<State> {
    LEGACY_STATE.get_or_init(|| RwLock::new(State::default()))
}

#[deprecated(note = "singleton elimination: use actor-owned error slot")]
#[allow(dead_code, deprecated)]
fn last_error_slot() -> &'static RwLock<String> {
    LEGACY_LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

#[deprecated(note = "singleton elimination: use actor-owned StoreConfig")]
#[allow(dead_code, deprecated)]
pub fn store_config() -> &'static RwLock<StoreConfig> {
    LEGACY_STORE_CONFIG.get_or_init(|| RwLock::new(StoreConfig::default()))
}

#[allow(dead_code, deprecated)]
pub fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_slot().write() {
        *slot = msg.into();
    }
}

#[allow(dead_code, deprecated)]
pub fn clear_last_error() {
    if let Ok(mut slot) = last_error_slot().write() {
        slot.clear();
    }
}

#[allow(dead_code, deprecated)]
pub fn last_error_message() -> String {
    last_error_slot()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "harmonic matrix error lock poisoned".to_string())
}

#[allow(dead_code)]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[allow(dead_code)]
pub fn history_limit() -> usize {
    harmonia_config_store::get_own("harmonic-matrix", "history-limit")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(4096)
}

#[allow(dead_code)]
pub fn truncate_payload(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect::<String>()
}

#[allow(dead_code)]
pub fn push_limited<T>(v: &mut Vec<T>, item: T, limit: usize) {
    v.push(item);
    if v.len() > limit {
        let over = v.len() - limit;
        v.drain(0..over);
    }
}

#[allow(dead_code)]
pub fn bump_revision(st: &mut State) {
    st.revision = st.revision.saturating_add(1);
}

#[allow(dead_code)]
pub fn reset_state(st: &mut State) {
    st.nodes.clear();
    st.edges.clear();
    st.plugged.clear();
    st.route_history.clear();
    st.events.clear();
    st.epoch = now_unix();
    st.revision = 1;
}
