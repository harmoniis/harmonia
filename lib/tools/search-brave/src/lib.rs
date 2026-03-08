use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

use harmonia_signal_integrity::wrap_secure;
use harmonia_vault::{get_secret_for_component, init_from_env};

const VERSION: &[u8] = b"harmonia-search-brave/0.1.0\0";

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

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
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn harmonia_search_brave_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_search_brave_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_search_brave_query(query: *const c_char) -> *mut c_char {
    let query = match cstr_to_string(query) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let _ = init_from_env();
    let key = match get_secret_for_component("search-brave-tool", "brave_api_key") {
        Ok(Some(v)) => v,
        Ok(None) => {
            set_error("missing secret: brave_api_key");
            return std::ptr::null_mut();
        }
        Err(e) => {
            set_error(format!("vault policy error: {e}"));
            return std::ptr::null_mut();
        }
    };

    let endpoint = env::var("HARMONIA_BRAVE_API_URL")
        .unwrap_or_else(|_| "https://api.search.brave.com/res/v1/web/search".to_string());

    let out = Command::new("curl")
        .arg("-sS")
        .arg("-G")
        .arg("-H")
        .arg(format!("X-Subscription-Token: {key}"))
        .arg("--data-urlencode")
        .arg(format!("q={query}"))
        .arg(endpoint)
        .output();

    match out {
        Ok(o) if o.status.success() => {
            clear_error();
            let raw = String::from_utf8_lossy(&o.stdout).to_string();
            let wrapped = wrap_secure(&raw, "search-brave");
            to_c_string(wrapped)
        }
        Ok(o) => {
            set_error(format!(
                "brave query failed: {}",
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

#[no_mangle]
pub extern "C" fn harmonia_search_brave_last_error() -> *mut c_char {
    to_c_string(
        last_error()
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "search-brave lock poisoned".to_string()),
    )
}

#[no_mangle]
pub extern "C" fn harmonia_search_brave_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
