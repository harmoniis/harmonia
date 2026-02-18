use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static SECRETS: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

pub fn secrets() -> &'static RwLock<HashMap<String, String>> {
    SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}

pub fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

pub fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}
