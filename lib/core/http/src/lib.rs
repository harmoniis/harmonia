use harmonia_vault::{get_secret_for_symbol, init_from_env};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-http/0.2.0\0";

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
    // Safety: caller provides valid null-terminated string.
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
pub extern "C" fn harmonia_http_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_http_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_http_request(method: *const c_char, url: *const c_char) -> *mut c_char {
    let method = match cstr_to_string(method) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg(method)
        .arg(url)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            clear_error();
            to_c_string(String::from_utf8_lossy(&out.stdout).to_string())
        }
        Ok(out) => {
            set_error(format!(
                "curl failed: {}",
                String::from_utf8_lossy(&out.stderr)
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
pub extern "C" fn harmonia_http_request_with_auth_symbol(
    method: *const c_char,
    url: *const c_char,
    auth_symbol: *const c_char,
) -> *mut c_char {
    let method = match cstr_to_string(method) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let auth_symbol = match cstr_to_string(auth_symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let _ = init_from_env();
    let secret = match get_secret_for_symbol(&auth_symbol) {
        Some(v) => v,
        None => {
            set_error(format!("missing secret for symbol: {auth_symbol}"));
            return std::ptr::null_mut();
        }
    };

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg(method)
        .arg("-H")
        .arg(format!("Authorization: Bearer {secret}"))
        .arg(url)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            clear_error();
            to_c_string(String::from_utf8_lossy(&out.stdout).to_string())
        }
        Ok(out) => {
            set_error(format!(
                "curl failed: {}",
                String::from_utf8_lossy(&out.stderr)
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
pub extern "C" fn harmonia_http_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "http lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_http_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer comes from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_http_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_http_version().is_null());
    }
}
