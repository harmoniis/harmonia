use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{State, StoreConfig};

#[allow(dead_code)]
static STATE: OnceLock<RwLock<State>> = OnceLock::new();
#[allow(dead_code)]
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
#[allow(dead_code)]
static STORE_CONFIG: OnceLock<RwLock<StoreConfig>> = OnceLock::new();

#[allow(dead_code)]
pub fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| RwLock::new(State::default()))
}

#[allow(dead_code)]
fn last_error_slot() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

#[allow(dead_code)]
pub fn store_config() -> &'static RwLock<StoreConfig> {
    STORE_CONFIG.get_or_init(|| RwLock::new(StoreConfig::default()))
}

#[allow(dead_code)]
pub fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_slot().write() {
        *slot = msg.into();
    }
}

#[allow(dead_code)]
pub fn clear_last_error() {
    if let Ok(mut slot) = last_error_slot().write() {
        slot.clear();
    }
}

#[allow(dead_code)]
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
