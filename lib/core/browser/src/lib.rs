use harmonia_vault::{get_secret_for_symbol, init_from_env};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-browser/0.3.0\0";

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
pub extern "C" fn harmonia_browser_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_browser_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_browser_fetch_title(url: *const c_char) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let output = Command::new("curl").arg("-sS").arg(&url).output();
    let body = match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        Ok(out) => {
            set_error(format!(
                "curl failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
            return std::ptr::null_mut();
        }
        Err(e) => {
            set_error(format!("curl exec failed: {e}"));
            return std::ptr::null_mut();
        }
    };
    let lower = body.to_lowercase();
    let start = match lower.find("<title>") {
        Some(v) => v + 7,
        None => {
            set_error("title not found".to_string());
            return std::ptr::null_mut();
        }
    };
    let end_rel = match lower[start..].find("</title>") {
        Some(v) => v,
        None => {
            set_error("title end not found".to_string());
            return std::ptr::null_mut();
        }
    };
    clear_error();
    to_c_string(body[start..start + end_rel].trim().to_string())
}

#[no_mangle]
pub extern "C" fn harmonia_browser_fetch_html(url: *const c_char) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let output = Command::new("curl").arg("-sS").arg(&url).output();
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
pub extern "C" fn harmonia_browser_fetch_html_with_auth_symbol(
    url: *const c_char,
    auth_symbol: *const c_char,
) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let symbol = match cstr_to_string(auth_symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let _ = init_from_env();
    let secret = match get_secret_for_symbol(&symbol) {
        Some(v) => v,
        None => {
            set_error(format!("missing secret for symbol: {symbol}"));
            return std::ptr::null_mut();
        }
    };
    let output = Command::new("curl")
        .arg("-sS")
        .arg("-H")
        .arg(format!("Authorization: Bearer {secret}"))
        .arg(&url)
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
pub extern "C" fn harmonia_browser_extract_links(url: *const c_char) -> *mut c_char {
    let html_ptr = harmonia_browser_fetch_html(url);
    if html_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let html = unsafe { CStr::from_ptr(html_ptr) }
        .to_string_lossy()
        .into_owned();
    harmonia_browser_free_string(html_ptr);

    let mut out = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0usize;
    while i + 6 < bytes.len() {
        if html[i..].starts_with("href=\"") {
            let start = i + 6;
            if let Some(end_rel) = html[start..].find('"') {
                let link = &html[start..start + end_rel];
                if !link.is_empty() {
                    out.push(link.to_string());
                }
                i = start + end_rel + 1;
                continue;
            }
        }
        i += 1;
    }
    clear_error();
    to_c_string(format!("({})", out.join(" ")))
}

#[no_mangle]
pub extern "C" fn harmonia_browser_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "browser lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_browser_free_string(ptr: *mut c_char) {
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
        assert_eq!(harmonia_browser_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_browser_version().is_null());
    }
}
