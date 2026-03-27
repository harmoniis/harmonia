use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

use harmonia_signal_integrity::wrap_secure;
use harmonia_vault::{get_secret_for_component, init_from_env};

const COMPONENT: &str = "search-exa-tool";
const VERSION: &[u8] = b"harmonia-search-exa/0.1.0\0";

/// Deprecated: legacy global singleton. Will be replaced by returning Result<T, String>.
static LEGACY_LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error() -> &'static RwLock<String> {
    LEGACY_LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
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
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

pub fn harmonia_search_exa_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_search_exa_healthcheck() -> i32 {
    1
}

pub fn harmonia_search_exa_query(query: *const c_char) -> *mut c_char {
    let query = match cstr_to_string(query) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let _ = init_from_env();
    let key = match get_secret_for_component("search-exa-tool", "exa_api_key") {
        Ok(Some(v)) => v,
        Ok(None) => {
            set_error("missing secret: exa_api_key");
            return std::ptr::null_mut();
        }
        Err(e) => {
            set_error(format!("vault policy error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let endpoint = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.exa.ai/search".to_string());
    let payload = format!("{{\"query\":\"{}\",\"numResults\":5}}", json_escape(&query));

    let out = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("x-api-key: {key}"))
        .arg("-d")
        .arg(payload)
        .arg(endpoint)
        .output();

    match out {
        Ok(o) if o.status.success() => {
            clear_error();
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let wrapped = wrap_secure(&raw, "search-exa");
            to_c_string(wrapped)
        }
        Ok(o) => {
            set_error(format!(
                "exa query failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
            std::ptr::null_mut()
        }
        Err(e) => {
            set_error(format!("curl exec failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

pub fn harmonia_search_exa_last_error() -> *mut c_char {
    to_c_string(
        last_error()
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "search-exa lock poisoned".to_string()),
    )
}

pub fn harmonia_search_exa_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
