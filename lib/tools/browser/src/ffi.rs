//! C-ABI exports for Lisp CFFI integration.
//!
//! Exports two sets of symbols:
//! - `harmonia_browser_*` — legacy browser-specific FFI
//! - `harmonia_tool_*`    — standardised ToolVtable contract
//!
//! All functions are synchronous extern "C" exports that block on the
//! internal tokio runtime when async operations are needed.

use crate::{chrome, engine, mcp, security};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-browser/2.1.0\0";

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

// ======================================================================
// ToolVtable-compatible exports (harmonia_tool_*)
// ======================================================================

/// ToolVtable: version string. Pointer is static, do NOT free.
#[no_mangle]
pub extern "C" fn harmonia_tool_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

/// ToolVtable: health check. Returns 1 if operational.
#[no_mangle]
pub extern "C" fn harmonia_tool_healthcheck() -> i32 {
    let _ = runtime();
    1
}

/// ToolVtable: initialize with s-expression config.
///
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_tool_init(config_sexp: *const c_char) -> i32 {
    let config = match cstr_to_string(config_sexp) {
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

/// ToolVtable: invoke a tool operation.
///
/// Dispatches based on operation name:
/// - "search"           — fetch URL + extract with macro
/// - "execute"          — multi-step browser plan
/// - "controlled-fetch" — SSRF-safe HTTP fetch
/// - "navigate"         — Chrome CDP navigate + return HTML
/// - "click"            — click element by CSS selector
/// - "type"             — type text into element
/// - "wait-for"         — wait for CSS selector to appear
/// - "screenshot"       — take viewport screenshot (base64 PNG)
/// - "get-text"         — get text content of element
/// - "evaluate-js"      — evaluate JavaScript expression
///
/// params_sexp format varies by operation (s-expression).
/// Returns result as s-expression string. Caller must free.
#[no_mangle]
pub extern "C" fn harmonia_tool_invoke(
    operation: *const c_char,
    params_sexp: *const c_char,
) -> *mut c_char {
    let op = match cstr_to_string(operation) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let params = match cstr_to_string(params_sexp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let result = dispatch_invoke(&op, &params);

    match result {
        Ok(output) => {
            clear_error();
            to_c_string(output)
        }
        Err(e) => {
            set_error(&e);
            to_c_string(format!("(:error \"{}\")", e.replace('"', "\\\"")))
        }
    }
}

/// ToolVtable: self-describing capabilities.
///
/// Returns s-expression listing all supported operations.
/// Caller must free.
#[no_mangle]
pub extern "C" fn harmonia_tool_capabilities() -> *mut c_char {
    to_c_string(CAPABILITIES_SEXP.to_string())
}

/// ToolVtable: last error message. Caller must free.
#[no_mangle]
pub extern "C" fn harmonia_tool_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "browser lock poisoned".to_string());
    to_c_string(msg)
}

/// ToolVtable: graceful shutdown. Returns 0.
#[no_mangle]
pub extern "C" fn harmonia_tool_shutdown() -> i32 {
    // No persistent state to clean up in the basic case.
    // Session pool cleanup would go here if we add global state.
    0
}

/// ToolVtable: free a string returned by this tool. Same as browser_free_string.
#[no_mangle]
pub extern "C" fn harmonia_tool_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr)) };
}

// ---- Invoke dispatch ----

fn dispatch_invoke(operation: &str, params: &str) -> Result<String, String> {
    match operation {
        "search" => dispatch_search(params),
        "execute" => dispatch_execute(params),
        "controlled-fetch" => dispatch_controlled_fetch(params),
        "navigate" => dispatch_navigate(params),
        "click" => dispatch_click(params),
        "type" => dispatch_type(params),
        "wait-for" => dispatch_wait_for(params),
        "screenshot" => dispatch_screenshot(params),
        "get-text" => dispatch_get_text(params),
        "evaluate-js" => dispatch_evaluate_js(params),
        _ => Err(format!("unknown operation: {}", operation)),
    }
}

fn dispatch_search(params: &str) -> Result<String, String> {
    let url = parse_sexp_field(params, ":url").ok_or("missing :url in params")?;
    let macro_name = parse_sexp_field(params, ":macro").unwrap_or_else(|| "text".to_string());
    let arg = parse_sexp_field(params, ":arg").unwrap_or_default();

    let result = runtime().block_on(async { mcp::browser_search(&url, &macro_name, &arg) });
    Ok(result)
}

fn dispatch_execute(params: &str) -> Result<String, String> {
    // params is either raw JSON steps or wrapped in :steps
    let steps = parse_sexp_field(params, ":steps").unwrap_or_else(|| params.to_string());
    let result = runtime().block_on(async { mcp::browser_execute(&steps) });
    Ok(result)
}

fn dispatch_controlled_fetch(params: &str) -> Result<String, String> {
    let url = parse_sexp_field(params, ":url").ok_or("missing :url in params")?;
    let method = parse_sexp_field(params, ":method").unwrap_or_else(|| "GET".to_string());
    let body = parse_sexp_field(params, ":body");

    let result = mcp::browser_controlled_fetch(&url, &method, body.as_deref());
    Ok(result)
}

fn dispatch_navigate(params: &str) -> Result<String, String> {
    let url = parse_sexp_field(params, ":url").ok_or("missing :url in params")?;

    let config = chrome::ChromeConfig::default();
    let html = chrome::chrome_fetch(&url, &config)?;
    let cleaned = engine::strip_scripts_and_styles(&html);
    let wrapped = security::wrap_secure(&serde_json::json!(cleaned), "navigate");
    Ok(wrapped)
}

#[cfg(feature = "chrome")]
fn dispatch_click(params: &str) -> Result<String, String> {
    let selector = parse_sexp_field(params, ":selector").ok_or("missing :selector in params")?;
    let url =
        parse_sexp_field(params, ":url").ok_or("missing :url — navigate first or provide URL")?;

    let config = chrome::ChromeConfig::default();
    // For DOM operations, we need a live tab — launch Chrome, navigate, then act
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;
    crate::stealth::cdp::click_element(&tab, &selector)?;

    Ok(format!(
        "(:ok :clicked \"{}\")",
        selector.replace('"', "\\\"")
    ))
}

#[cfg(not(feature = "chrome"))]
fn dispatch_click(_params: &str) -> Result<String, String> {
    Err("click requires Chrome CDP: compile with --features chrome".to_string())
}

#[cfg(feature = "chrome")]
fn dispatch_type(params: &str) -> Result<String, String> {
    let selector = parse_sexp_field(params, ":selector").ok_or("missing :selector in params")?;
    let text = parse_sexp_field(params, ":text").ok_or("missing :text in params")?;
    let url =
        parse_sexp_field(params, ":url").ok_or("missing :url — navigate first or provide URL")?;

    let config = chrome::ChromeConfig::default();
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;
    crate::stealth::cdp::type_text(&tab, &selector, &text)?;

    Ok(format!(
        "(:ok :typed {} :into \"{}\")",
        text.len(),
        selector.replace('"', "\\\"")
    ))
}

#[cfg(not(feature = "chrome"))]
fn dispatch_type(_params: &str) -> Result<String, String> {
    Err("type requires Chrome CDP: compile with --features chrome".to_string())
}

#[cfg(feature = "chrome")]
fn dispatch_wait_for(params: &str) -> Result<String, String> {
    let selector = parse_sexp_field(params, ":selector").ok_or("missing :selector in params")?;
    let url = parse_sexp_field(params, ":url").ok_or("missing :url")?;
    let timeout: u64 = parse_sexp_field(params, ":timeout-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000);

    let config = chrome::ChromeConfig::default();
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;
    crate::stealth::cdp::wait_for_selector(&tab, &selector, timeout)?;

    Ok(format!(
        "(:ok :found \"{}\")",
        selector.replace('"', "\\\"")
    ))
}

#[cfg(not(feature = "chrome"))]
fn dispatch_wait_for(_params: &str) -> Result<String, String> {
    Err("wait-for requires Chrome CDP: compile with --features chrome".to_string())
}

#[cfg(feature = "chrome")]
fn dispatch_screenshot(params: &str) -> Result<String, String> {
    let url = parse_sexp_field(params, ":url").ok_or("missing :url")?;

    let config = chrome::ChromeConfig::default();
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;

    let b64 = crate::stealth::cdp::screenshot(&tab)?;
    Ok(format!(
        "(:ok :format \"png\" :encoding \"base64\" :data \"{}\")",
        b64
    ))
}

#[cfg(not(feature = "chrome"))]
fn dispatch_screenshot(_params: &str) -> Result<String, String> {
    Err("screenshot requires Chrome CDP: compile with --features chrome".to_string())
}

#[cfg(feature = "chrome")]
fn dispatch_get_text(params: &str) -> Result<String, String> {
    let selector = parse_sexp_field(params, ":selector").ok_or("missing :selector")?;
    let url = parse_sexp_field(params, ":url").ok_or("missing :url")?;

    let config = chrome::ChromeConfig::default();
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;

    let text = crate::stealth::cdp::get_text(&tab, &selector)?;
    let wrapped = security::wrap_secure(&serde_json::json!(text), "get_text");
    Ok(wrapped)
}

#[cfg(not(feature = "chrome"))]
fn dispatch_get_text(_params: &str) -> Result<String, String> {
    Err("get-text requires Chrome CDP: compile with --features chrome".to_string())
}

#[cfg(feature = "chrome")]
fn dispatch_evaluate_js(params: &str) -> Result<String, String> {
    let expression = parse_sexp_field(params, ":expression").ok_or("missing :expression")?;
    let url = parse_sexp_field(params, ":url").ok_or("missing :url")?;

    let config = chrome::ChromeConfig::default();
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_opts = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_opts).map_err(|e| format!("failed to launch Chrome: {}", e))?;
    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;
    tab.navigate_to(&url)
        .map_err(|e| format!("navigation failed: {}", e))?;
    tab.wait_until_navigated()
        .map_err(|e| format!("wait failed: {}", e))?;
    crate::stealth::page_load_delay();
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;

    let result = crate::stealth::cdp::evaluate_js(&tab, &expression)?;
    let wrapped = security::wrap_secure(&serde_json::json!(result), "evaluate_js");
    Ok(wrapped)
}

#[cfg(not(feature = "chrome"))]
fn dispatch_evaluate_js(_params: &str) -> Result<String, String> {
    Err("evaluate-js requires Chrome CDP: compile with --features chrome".to_string())
}

/// Parse a simple s-expression field like `:key "value"` or `:key value`.
fn parse_sexp_field(sexp: &str, key: &str) -> Option<String> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if after.starts_with('"') {
        let rest = &after[1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    } else {
        // Unquoted value: read until whitespace or closing paren
        let end = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        let val = &after[..end];
        if val.is_empty() {
            None
        } else {
            Some(val.to_string())
        }
    }
}

/// Capabilities s-expression for the browser tool.
const CAPABILITIES_SEXP: &str = r#"((:operation "search"
  :description "Fetch URL and extract data with named macro"
  :params ((:name "url" :kind "string" :required t)
           (:name "macro" :kind "string" :required nil)
           (:name "arg" :kind "string" :required nil)))
 (:operation "execute"
  :description "Multi-step browser plan (JSON array of steps)"
  :params ((:name "steps" :kind "json" :required t)))
 (:operation "controlled-fetch"
  :description "SSRF-safe HTTP fetch with security wrapping"
  :params ((:name "url" :kind "string" :required t)
           (:name "method" :kind "string" :required nil)
           (:name "body" :kind "string" :required nil)))
 (:operation "navigate"
  :description "Navigate Chrome to URL and return rendered HTML"
  :params ((:name "url" :kind "string" :required t)))
 (:operation "click"
  :description "Click element by CSS selector (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)
           (:name "selector" :kind "string" :required t)))
 (:operation "type"
  :description "Type text into element (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)
           (:name "selector" :kind "string" :required t)
           (:name "text" :kind "string" :required t)))
 (:operation "wait-for"
  :description "Wait for CSS selector to appear in DOM (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)
           (:name "selector" :kind "string" :required t)
           (:name "timeout-ms" :kind "integer" :required nil)))
 (:operation "screenshot"
  :description "Take viewport screenshot as base64 PNG (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)))
 (:operation "get-text"
  :description "Get text content of element by CSS selector (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)
           (:name "selector" :kind "string" :required t)))
 (:operation "evaluate-js"
  :description "Evaluate JavaScript expression in page (requires Chrome)"
  :params ((:name "url" :kind "string" :required t)
           (:name "expression" :kind "string" :required t))))"#;

// ======================================================================
// Legacy browser-specific exports (harmonia_browser_*)
// ======================================================================

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
    fn tool_healthcheck_returns_one() {
        assert_eq!(harmonia_tool_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        let ptr = harmonia_browser_version();
        assert!(!ptr.is_null());
        let version = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        assert!(version.starts_with("harmonia-browser/"));
    }

    #[test]
    fn tool_version_matches_browser_version() {
        let browser_v = harmonia_browser_version();
        let tool_v = harmonia_tool_version();
        let bv = unsafe { CStr::from_ptr(browser_v) }.to_string_lossy();
        let tv = unsafe { CStr::from_ptr(tool_v) }.to_string_lossy();
        assert_eq!(bv, tv);
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
    fn tool_init_with_empty_config() {
        let config = CString::new("()").unwrap();
        let result = harmonia_tool_init(config.as_ptr());
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

    #[test]
    fn tool_capabilities_is_non_null() {
        let ptr = harmonia_tool_capabilities();
        assert!(!ptr.is_null());
        let caps = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_tool_free_string(ptr);
        assert!(caps.contains("search"));
        assert!(caps.contains("navigate"));
        assert!(caps.contains("controlled-fetch"));
        assert!(caps.contains("click"));
        assert!(caps.contains("evaluate-js"));
    }

    #[test]
    fn tool_invoke_unknown_operation() {
        let op = CString::new("nonexistent").unwrap();
        let params = CString::new("()").unwrap();
        let ptr = harmonia_tool_invoke(op.as_ptr(), params.as_ptr());
        assert!(!ptr.is_null());
        let result = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_tool_free_string(ptr);
        assert!(result.contains("unknown operation"));
    }

    #[test]
    fn parse_sexp_field_quoted() {
        let sexp = r#"(:url "https://example.com" :macro "text")"#;
        assert_eq!(
            parse_sexp_field(sexp, ":url"),
            Some("https://example.com".to_string())
        );
        assert_eq!(parse_sexp_field(sexp, ":macro"), Some("text".to_string()));
    }

    #[test]
    fn parse_sexp_field_unquoted() {
        let sexp = "(:timeout-ms 5000 :retry t)";
        assert_eq!(
            parse_sexp_field(sexp, ":timeout-ms"),
            Some("5000".to_string())
        );
        assert_eq!(parse_sexp_field(sexp, ":retry"), Some("t".to_string()));
    }

    #[test]
    fn parse_sexp_field_missing() {
        let sexp = "(:url \"test\")";
        assert_eq!(parse_sexp_field(sexp, ":missing"), None);
    }

    #[test]
    fn tool_shutdown_returns_zero() {
        assert_eq!(harmonia_tool_shutdown(), 0);
    }
}
