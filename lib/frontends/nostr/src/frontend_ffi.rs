use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

use crate::client;

const VERSION: &[u8] = b"harmonia-nostr/0.2.0\0";

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}
fn set_error(msg: impl Into<String>) {
    if let Ok(mut s) = last_error().write() {
        *s = msg.into();
    }
}
fn clear_error() {
    if let Ok(mut s) = last_error().write() {
        s.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".into());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

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
    let config_str = match cstr_to_string(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match client::init(&config_str) {
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
        set_error("poll: null buffer or zero length");
        return -1;
    }

    let signals = match client::poll() {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    if signals.is_empty() {
        clear_error();
        return 0;
    }

    // Format: sub_channel\tpayload[\tmetadata]\n per signal
    let mut output = String::new();
    for (sub, payload, metadata) in &signals {
        output.push_str(sub);
        output.push('\t');
        output.push_str(payload);
        if let Some(meta) = metadata {
            output.push('\t');
            output.push_str(meta);
        }
        output.push('\n');
    }

    let bytes = output.as_bytes();
    let to_write = bytes.len().min(buf_len.saturating_sub(1));
    if to_write == 0 {
        return 0;
    }

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, to_write);
        *((buf as *mut u8).add(to_write)) = 0;
    }

    clear_error();
    to_write as i32
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let channel_str = match cstr_to_string(channel) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let payload_str = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match client::send(&channel_str, &payload_str) {
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
    let s = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "nostr lock poisoned".into());
    CString::new(s)
        .map(|c| c.into_raw() as *const c_char)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;

    #[test]
    fn test_healthcheck_before_init() {
        assert_eq!(harmonia_frontend_healthcheck(), 0);
    }

    #[test]
    fn test_version() {
        let ptr = harmonia_frontend_version();
        assert!(!ptr.is_null());
        let v = unsafe { CStr::from_ptr(ptr) }.to_str().unwrap();
        assert_eq!(v, "harmonia-nostr/0.2.0");
    }
}
