use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-vault/0.2.0\0";

static SECRETS: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn secrets() -> &'static RwLock<HashMap<String, String>> {
    SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }

    // Safety: caller must provide valid null-terminated C string.
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

pub fn init_from_env() -> Result<(), String> {
    let mut secrets = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;

    secrets.clear();

    if let Ok(key) = env::var("OPENROUTER_API_KEY") {
        secrets.insert("openrouter".to_string(), key);
    }

    Ok(())
}

pub fn get_secret_for_symbol(symbol: &str) -> Option<String> {
    let normalized = symbol.trim().trim_start_matches(':').to_ascii_lowercase();
    secrets()
        .read()
        .ok()
        .and_then(|map| map.get(&normalized).cloned())
}

#[no_mangle]
pub extern "C" fn harmonia_vault_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_vault_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_vault_init() -> i32 {
    match init_from_env() {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_vault_get_secret(symbol: *const c_char) -> *mut c_char {
    let symbol = match cstr_to_string(symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match get_secret_for_symbol(&symbol) {
        Some(secret) => {
            clear_error();
            to_c_string(secret)
        }
        None => {
            set_error(format!("secret not found: {symbol}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_vault_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "vault lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_vault_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // Safety: ptr must come from CString::into_raw from this crate.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_vault_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_vault_version().is_null());
    }

    #[test]
    fn normalize_symbol_lookup() {
        {
            let mut map = secrets().write().unwrap();
            map.insert("openrouter".to_string(), "k".to_string());
        }
        assert_eq!(get_secret_for_symbol(":OpenRouter").as_deref(), Some("k"));
    }
}
