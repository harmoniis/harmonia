//! ToolVtable FFI exports for the Zoom tool.
//!
//! Implements the standard `harmonia_tool_*` contract so the gateway
//! can load and invoke Zoom operations uniformly.

use crate::operations;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-zoom/0.1.0\0";

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

// ---- ToolVtable exports ----

/// Version string. Pointer is static, do NOT free.
pub fn harmonia_tool_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

/// Health check. Returns 1 if operational.
pub fn harmonia_tool_healthcheck() -> i32 {
    1
}

/// Initialize with s-expression config. Returns 0 on success.
pub fn harmonia_tool_init(_config_sexp: *const c_char) -> i32 {
    clear_error();
    0
}

/// Invoke a Zoom operation.
///
/// Dispatches to the appropriate operation handler based on the operation name.
/// Caller must free the returned string with `harmonia_tool_free_string`.
pub fn harmonia_tool_invoke(operation: *const c_char, params_sexp: *const c_char) -> *mut c_char {
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

    match operations::zoom_invoke(&op, &params) {
        Ok(result) => {
            clear_error();
            to_c_string(result)
        }
        Err(e) => {
            set_error(&e);
            to_c_string(format!("(:error \"{}\")", e.replace('"', "\\\"")))
        }
    }
}

/// Self-describing capabilities. Caller must free.
pub fn harmonia_tool_capabilities() -> *mut c_char {
    to_c_string(operations::zoom_capabilities().to_string())
}

/// Last error message. Caller must free.
pub fn harmonia_tool_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "zoom lock poisoned".to_string());
    to_c_string(msg)
}

/// Graceful shutdown. Returns 0.
pub fn harmonia_tool_shutdown() -> i32 {
    0
}

/// Free a string returned by this tool.
pub fn harmonia_tool_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_null() {
        let ptr = harmonia_tool_version();
        assert!(!ptr.is_null());
        let v = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        assert!(v.starts_with("harmonia-zoom/"));
    }

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_tool_healthcheck(), 1);
    }

    #[test]
    fn capabilities_contains_operations() {
        let ptr = harmonia_tool_capabilities();
        assert!(!ptr.is_null());
        let caps = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_tool_free_string(ptr);
        assert!(caps.contains("join"));
        assert!(caps.contains("leave"));
    }

    #[test]
    fn invoke_unknown_returns_error() {
        let op = CString::new("nonexistent").unwrap();
        let params = CString::new("()").unwrap();
        let ptr = harmonia_tool_invoke(op.as_ptr(), params.as_ptr());
        assert!(!ptr.is_null());
        let result = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_tool_free_string(ptr);
        assert!(result.contains(":error"));
    }

    #[test]
    fn shutdown_returns_zero() {
        assert_eq!(harmonia_tool_shutdown(), 0);
    }
}
