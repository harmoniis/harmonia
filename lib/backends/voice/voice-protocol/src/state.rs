use std::sync::{OnceLock, RwLock};

/// Deprecated: legacy global singleton. Will be replaced by returning Result<T, String>.
static LEGACY_LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn error_lock() -> &'static RwLock<String> {
    LEGACY_LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: impl Into<String>) {
    if let Ok(mut s) = error_lock().write() {
        *s = msg.into();
    }
}

pub fn clear_error() {
    if let Ok(mut s) = error_lock().write() {
        s.clear();
    }
}

pub fn last_error_message() -> String {
    error_lock()
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "voice protocol lock poisoned".into())
}
