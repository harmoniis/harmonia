use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::baseband::{poll_baseband, send_signal};
use crate::model::SecurityLabel;
use crate::state::{clear_error, gateway, init, last_error, set_error};

const VERSION: &[u8] = b"harmonia-gateway/0.1.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_init() -> i32 {
    match init() {
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
pub extern "C" fn harmonia_gateway_register(
    name: *const c_char,
    so_path: *const c_char,
    config_sexp: *const c_char,
    security_label: *const c_char,
) -> i32 {
    let name = match cstr_to_string(name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let so_path = match cstr_to_string(so_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let config_sexp = match cstr_to_string(config_sexp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let security_label = match cstr_to_string(security_label) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let label = SecurityLabel::from_str(&security_label);
    let gw = gateway();
    match gw.registry.register(&name, &so_path, &config_sexp, label) {
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
pub extern "C" fn harmonia_gateway_unregister(name: *const c_char) -> i32 {
    let name = match cstr_to_string(name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let gw = gateway();
    match gw.registry.unregister(&name) {
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
pub extern "C" fn harmonia_gateway_poll() -> *mut c_char {
    let gw = gateway();
    let batch = poll_baseband(&gw.registry);
    clear_error();
    to_c_string(batch.to_sexp())
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_send(
    frontend_name: *const c_char,
    sub_channel: *const c_char,
    payload: *const c_char,
) -> i32 {
    let frontend_name = match cstr_to_string(frontend_name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let sub_channel = match cstr_to_string(sub_channel) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let payload = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let gw = gateway();
    match send_signal(&gw.registry, &frontend_name, &sub_channel, &payload) {
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
pub extern "C" fn harmonia_gateway_list_frontends() -> *mut c_char {
    let gw = gateway();
    let names = gw.registry.list_names();
    clear_error();
    if names.is_empty() {
        to_c_string("nil".to_string())
    } else {
        let items: Vec<String> = names.iter().map(|n| format!("\"{}\"", n)).collect();
        to_c_string(format!("({})", items.join(" ")))
    }
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_frontend_status(name: *const c_char) -> *mut c_char {
    let name = match cstr_to_string(name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let gw = gateway();
    match gw.registry.with_frontend(&name, |handle| {
        let version = handle.vtable.version();
        let healthy = handle.vtable.healthcheck();
        let caps = handle.capabilities_sexp();
        format!(
            "(:name \"{}\" :version \"{}\" :healthy {} :security \"{}\" :capabilities {})",
            name,
            version,
            if healthy { "t" } else { "nil" },
            handle.security_label.as_str(),
            caps,
        )
    }) {
        Ok(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_list_channels(name: *const c_char) -> *mut c_char {
    let name = match cstr_to_string(name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let gw = gateway();
    match gw.registry.with_frontend(&name, |handle| {
        handle
            .vtable
            .list_channels()
            .unwrap_or_else(|| "nil".to_string())
    }) {
        Ok(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_shutdown() -> i32 {
    let gw = gateway();
    gw.registry.shutdown_all();
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: ptr must come from CString::into_raw from this crate.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[no_mangle]
pub extern "C" fn harmonia_gateway_last_error() -> *mut c_char {
    let msg = last_error().read().clone();
    to_c_string(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_gateway_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_gateway_version().is_null());
    }

    #[test]
    fn init_succeeds() {
        assert_eq!(harmonia_gateway_init(), 0);
    }
}
