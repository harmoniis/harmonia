//! C-ABI exports for Lisp CFFI integration.
//!
//! All functions are synchronous extern "C" exports that block on the
//! internal tokio runtime when async operations are needed.

use crate::{chrome, engine, mcp, security};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-browser/2.0.0\0";

// ---- Error handling ----

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

// ---- FFI utilities ----

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

// ---- Tokio runtime for async operations ----

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for browser")
    })
}

// ---- Core exports ----

/// Returns the version string. Pointer is static, do NOT free.
#[no_mangle]
pub extern "C" fn harmonia_browser_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

/// Health check. Returns 1 if the browser tool is operational.
#[no_mangle]
pub extern "C" fn harmonia_browser_healthcheck() -> i32 {
    let _ = runtime();
    1
}

/// Initialize the browser engine with an s-expression config string.
///
/// Config format: `(:timeout 10000 :user-agent "harmonia/2.0" :allowlist ("example.com"))`
/// Returns 0 on success, -1 on error (check harmonia_browser_last_error).
#[no_mangle]
pub extern "C" fn harmonia_browser_init(config: *const c_char) -> i32 {
    let config = match cstr_to_string(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match engine::init(&config) {
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

// ---- 2-Tool MCP Surface ----

/// MCP tool: browser_search(url, macro, arg) -> security-wrapped s-expression.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_search(
    url: *const c_char,
    macro_name: *const c_char,
    arg: *const c_char,
) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let macro_name = match cstr_to_string(macro_name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let arg = match cstr_to_string(arg) {
        Ok(v) => v,
        Err(_) => String::new(),
    };

    let result = runtime().block_on(async { mcp::browser_search(&url, &macro_name, &arg) });

    clear_error();
    to_c_string(result)
}

/// MCP tool: browser_execute(steps_json) -> security-wrapped results.
///
/// steps_json is a JSON array: `[{"url":"...","macro":"text","arg":""}]`
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_execute(steps_json: *const c_char) -> *mut c_char {
    let steps = match cstr_to_string(steps_json) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let result = runtime().block_on(async { mcp::browser_execute(&steps) });

    clear_error();
    to_c_string(result)
}

// ---- Controlled Fetch (SSRF-safe API calls) ----

/// Controlled HTTP fetch: blocks dangerous targets, enforces domain allowlist.
///
/// This is the AgentBrowser.fetch() equivalent. Use for agent API calls.
/// method: "GET" or "POST"
/// body: JSON body for POST requests (NULL for GET)
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_controlled_fetch(
    url: *const c_char,
    method: *const c_char,
    body: *const c_char,
) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let method = match cstr_to_string(method) {
        Ok(v) => v,
        Err(_) => "GET".to_string(),
    };
    let body = cstr_to_string(body).ok();

    let result = mcp::browser_controlled_fetch(&url, &method, body.as_deref());

    clear_error();
    to_c_string(result)
}

// ---- Chrome CDP ----

/// Returns 1 if Chrome CDP is available (compiled with --features chrome), 0 otherwise.
#[no_mangle]
pub extern "C" fn harmonia_browser_chrome_available() -> i32 {
    if chrome::chrome_available() {
        1
    } else {
        0
    }
}

// ---- Legacy exports (backward compatible, now security-wrapped) ----

/// Legacy: fetch raw HTML from a URL. Result is security-wrapped.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_fetch_html(url: *const c_char) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match engine::fetch(&url) {
        Ok(html) => {
            let cleaned = engine::strip_scripts_and_styles(&html);
            clear_error();
            let wrapped = security::wrap_secure(&serde_json::json!(cleaned), "fetch_html");
            to_c_string(wrapped)
        }
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

/// Legacy: fetch a page title. Result is security-wrapped.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_fetch_title(url: *const c_char) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match engine::fetch(&url) {
        Ok(html) => {
            let title = engine::extract_title(&html).unwrap_or_default();
            clear_error();
            let wrapped = security::wrap_secure(&serde_json::json!(title), "fetch_title");
            to_c_string(wrapped)
        }
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

/// Legacy: extract links from a URL. Result is security-wrapped s-expression.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_extract_links(url: *const c_char) -> *mut c_char {
    let url = match cstr_to_string(url) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match engine::fetch(&url) {
        Ok(html) => {
            let links = engine::extract_links(&html);
            clear_error();
            let wrapped = security::wrap_secure(&serde_json::json!(links), "extract_links");
            to_c_string(wrapped)
        }
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

// ---- Security & discovery exports ----

/// Returns the agent security prompt text that should be included in system prompts.
///
/// Pointer is static, do NOT free.
#[no_mangle]
pub extern "C" fn harmonia_browser_security_prompt() -> *const c_char {
    static PROMPT: OnceLock<CString> = OnceLock::new();
    let cstr = PROMPT.get_or_init(|| {
        CString::new(security::agent_security_prompt()).expect("security prompt contains null byte")
    });
    cstr.as_ptr()
}

/// Returns the MCP tool definitions as a JSON string.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_mcp_tools() -> *mut c_char {
    let defs = mcp::mcp_tool_definitions();
    to_c_string(serde_json::to_string(&defs).unwrap_or_else(|_| "[]".to_string()))
}

// ---- Error & memory management ----

/// Returns the last error message.
///
/// Caller must free the returned string with `harmonia_browser_free_string`.
#[no_mangle]
pub extern "C" fn harmonia_browser_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "browser lock poisoned".to_string());
    to_c_string(msg)
}

/// Free a string previously returned by this crate.
///
/// # Safety
/// The pointer must have been returned by one of this crate's functions
/// and must not have been freed already.
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
        let ptr = harmonia_browser_version();
        assert!(!ptr.is_null());
        let version = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        assert_eq!(version, "harmonia-browser/2.0.0");
    }

    #[test]
    fn security_prompt_is_non_null() {
        let ptr = harmonia_browser_security_prompt();
        assert!(!ptr.is_null());
        let prompt = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        assert!(prompt.contains("SECURE BROWSER"));
    }

    #[test]
    fn mcp_tools_returns_json() {
        let ptr = harmonia_browser_mcp_tools();
        assert!(!ptr.is_null());
        let json_str = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_browser_free_string(ptr);
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    #[test]
    fn init_with_empty_config() {
        let config = CString::new("()").unwrap();
        let result = harmonia_browser_init(config.as_ptr());
        assert_eq!(result, 0);
    }

    #[test]
    fn null_url_returns_null() {
        let ptr = harmonia_browser_fetch_html(std::ptr::null());
        assert!(ptr.is_null());
    }

    #[test]
    fn chrome_available_returns_int() {
        let result = harmonia_browser_chrome_available();
        // Without feature, should be 0
        assert!(result == 0 || result == 1);
    }

    #[test]
    fn controlled_fetch_null_url_returns_null() {
        let ptr =
            harmonia_browser_controlled_fetch(std::ptr::null(), std::ptr::null(), std::ptr::null());
        assert!(ptr.is_null());
    }

    #[test]
    fn controlled_fetch_blocks_localhost() {
        let url = CString::new("http://localhost:8080/").unwrap();
        let method = CString::new("GET").unwrap();
        let ptr =
            harmonia_browser_controlled_fetch(url.as_ptr(), method.as_ptr(), std::ptr::null());
        assert!(!ptr.is_null());
        let result = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_browser_free_string(ptr);
        assert!(result.contains("BLOCKED"));
        assert!(result.contains("security-boundary"));
    }
}
