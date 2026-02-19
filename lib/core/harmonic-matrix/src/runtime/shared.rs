use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::model::{State, StoreConfig};

static STATE: OnceLock<RwLock<State>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
static STORE_CONFIG: OnceLock<RwLock<StoreConfig>> = OnceLock::new();

pub(super) fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| RwLock::new(State::default()))
}

fn last_error_slot() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub(super) fn store_config() -> &'static RwLock<StoreConfig> {
    STORE_CONFIG.get_or_init(|| RwLock::new(StoreConfig::default()))
}

pub(crate) fn set_last_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_slot().write() {
        *slot = msg.into();
    }
}

pub(crate) fn clear_last_error() {
    if let Ok(mut slot) = last_error_slot().write() {
        slot.clear();
    }
}

pub(crate) fn last_error_message() -> String {
    last_error_slot()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "harmonic matrix error lock poisoned".to_string())
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub(super) fn history_limit() -> usize {
    std::env::var("HARMONIA_MATRIX_HISTORY_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(4096)
}

pub(super) fn truncate_payload(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect::<String>()
}

pub(super) fn push_limited<T>(v: &mut Vec<T>, item: T, limit: usize) {
    v.push(item);
    if v.len() > limit {
        let over = v.len() - limit;
        v.drain(0..over);
    }
}

pub(super) fn bump_revision(st: &mut State) {
    st.revision = st.revision.saturating_add(1);
}

pub(super) fn reset_state(st: &mut State) {
    st.nodes.clear();
    st.edges.clear();
    st.plugged.clear();
    st.route_history.clear();
    st.events.clear();
    st.epoch = now_unix();
    st.revision = 1;
}
