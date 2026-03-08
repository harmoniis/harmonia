use std::sync::{OnceLock, RwLock};

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

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
        .unwrap_or_else(|_| "openrouter lock poisoned".to_string())
}
