use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-vault/0.3.0\0";

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

fn store_path() -> PathBuf {
    env::var("HARMONIA_VAULT_STORE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/harmonia/vault.secrets"))
}

fn normalize_symbol(symbol: &str) -> String {
    symbol.trim().trim_start_matches(':').to_ascii_lowercase()
}

fn normalize_env_symbol(raw: &str) -> String {
    normalize_symbol(&raw.to_ascii_lowercase().replace("__", "-"))
}

fn load_store_file() -> HashMap<String, String> {
    let path = store_path();
    let mut map = HashMap::new();
    if let Ok(body) = fs::read_to_string(path) {
        for line in body.lines() {
            if let Some((k, v)) = line.split_once('=') {
                map.insert(normalize_symbol(k), v.to_string());
            }
        }
    }
    map
}

fn persist_store_file(map: &HashMap<String, String>) -> Result<(), String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("vault store dir create failed: {e}"))?;
    }
    let mut out = String::new();
    for (k, v) in map {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    fs::write(path, out).map_err(|e| format!("vault store write failed: {e}"))?;
    Ok(())
}

pub fn init_from_env() -> Result<(), String> {
    let mut secrets = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;

    secrets.clear();
    for (k, v) in load_store_file() {
        secrets.insert(k, v);
    }

    // Generic, non-hardcoded vault ingest:
    // Any env var with HARMONIA_VAULT_SECRET__<SYMBOL>=<VALUE> becomes vault secret <symbol>.
    // Example:
    //   HARMONIA_VAULT_SECRET__OPENROUTER=sk-...
    //   HARMONIA_VAULT_SECRET__EXA_API_KEY=exa-...
    // Suffix normalization: lower-case + "__" => "-"
    for (k, v) in env::vars() {
        if let Some(symbol_raw) = k.strip_prefix("HARMONIA_VAULT_SECRET__") {
            let symbol = normalize_env_symbol(symbol_raw);
            if !symbol.is_empty() {
                secrets.insert(symbol, v);
            }
        }
    }

    // Backward compatibility alias for existing deployments.
    if let Ok(key) = env::var("OPENROUTER_API_KEY") {
        secrets.insert("openrouter".to_string(), key);
    }
    if let Ok(key) = env::var("EXA_API_KEY").or_else(|_| env::var("HARMONIA_EXA_API_KEY")) {
        secrets.insert("exa_api_key".to_string(), key.clone());
        secrets.insert("exa".to_string(), key);
    }
    if let Ok(key) = env::var("BRAVE_API_KEY")
        .or_else(|_| env::var("BRAVE_SEARCH_API_KEY"))
        .or_else(|_| env::var("HARMONIA_BRAVE_API_KEY"))
    {
        secrets.insert("brave_api_key".to_string(), key.clone());
        secrets.insert("brave".to_string(), key);
    }

    Ok(())
}

pub fn get_secret_for_symbol(symbol: &str) -> Option<String> {
    let normalized = normalize_symbol(symbol);
    secrets()
        .read()
        .ok()
        .and_then(|map| map.get(&normalized).cloned())
}

pub fn set_secret_for_symbol(symbol: &str, value: &str) -> Result<(), String> {
    let key = normalize_symbol(symbol);
    let mut map = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;
    map.insert(key, value.to_string());
    persist_store_file(&map)
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
    let _ = symbol;
    set_error("vault read denied over C API; use backend-internal vault access");
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn harmonia_vault_set_secret(symbol: *const c_char, value: *const c_char) -> i32 {
    let symbol = match cstr_to_string(symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let value = match cstr_to_string(value) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match set_secret_for_symbol(&symbol, &value) {
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
