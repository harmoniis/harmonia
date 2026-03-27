//! Thread-safe global error state for FFI backends.

use std::sync::{OnceLock, RwLock};

/// Deprecated: legacy global singleton. Will be replaced by returning Result<T, String>.
static LEGACY_LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn error_slot() -> &'static RwLock<String> {
    LEGACY_LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: String) {
    if let Ok(mut w) = error_slot().write() {
        *w = msg;
    }
}

pub fn clear_error() {
    if let Ok(mut w) = error_slot().write() {
        w.clear();
    }
}

pub fn last_error_message() -> String {
    error_slot().read().map(|r| r.clone()).unwrap_or_default()
}
