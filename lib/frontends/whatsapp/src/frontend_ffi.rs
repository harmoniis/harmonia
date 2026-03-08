use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

use crate::client;

const VERSION: &[u8] = b"harmonia-whatsapp/0.2.0\0";

// ---------------------------------------------------------------------------
// Error bookkeeping
// ---------------------------------------------------------------------------
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
fn last_error_rw() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}
fn set_error(msg: impl Into<String>) {
    if let Ok(mut s) = last_error_rw().write() {
        *s = msg.into();
    }
}
fn clear_error() {
    if let Ok(mut s) = last_error_rw().write() {
        s.clear();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
fn cstr_to_str(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".into());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// FFI exports
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn harmonia_frontend_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_healthcheck() -> i32 {
    if client::is_initialized() {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_init(config: *const c_char) -> i32 {
    let cfg = match cstr_to_str(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match client::init(&cfg) {
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
pub extern "C" fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        set_error("null or zero-length buffer");
        return -1;
    }
    match client::poll() {
        Ok(signals) => {
            if signals.is_empty() {
                clear_error();
                return 0;
            }
            // Format: newline-separated lines of `sub_channel\tpayload`.
            let formatted: String = signals
                .iter()
                .map(|(ch, pl)| format!("{}\t{}", ch, pl))
                .collect::<Vec<_>>()
                .join("\n");
            let bytes = formatted.as_bytes();
            let to_copy = bytes.len().min(buf_len.saturating_sub(1));
            unsafe {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, to_copy);
                *((buf as *mut u8).add(to_copy)) = 0; // NUL-terminate
            }
            clear_error();
            to_copy as i32
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let phone = match cstr_to_str(channel) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let text = match cstr_to_str(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match client::send(&phone, &text) {
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
pub extern "C" fn harmonia_frontend_last_error() -> *const c_char {
    let msg = last_error_rw()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "lock poisoned".into());
    CString::new(msg)
        .map(|s| s.into_raw() as *const c_char)
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_shutdown() -> i32 {
    client::shutdown();
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
