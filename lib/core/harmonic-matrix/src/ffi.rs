use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::runtime::{
    clear_last_error, init, last_error_message, log_event, observe_route, register_edge,
    register_node, report, route_allowed, route_timeseries, set_last_error, set_store,
    set_tool_enabled, time_report,
};

const VERSION: &[u8] = b"harmonia-harmonic-matrix/0.3.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn cstr_to_optional_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Some(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_set_store(
    kind: *const c_char,
    path: *const c_char,
) -> i32 {
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let path = cstr_to_optional_string(path);

    match set_store(&kind, path.as_deref()) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_get_store() -> *mut c_char {
    match crate::runtime::store_summary() {
        Ok(v) => {
            clear_last_error();
            to_c_string(v)
        }
        Err(e) => {
            set_last_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_init() -> i32 {
    match init() {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_register_node(
    node_id: *const c_char,
    kind: *const c_char,
) -> i32 {
    let node_id = match cstr_to_string(node_id) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };

    match register_node(&node_id, &kind) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_set_tool_enabled(
    tool_id: *const c_char,
    enabled: i32,
) -> i32 {
    let tool_id = match cstr_to_string(tool_id) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };

    match set_tool_enabled(&tool_id, enabled != 0) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_register_edge(
    from: *const c_char,
    to: *const c_char,
    weight: f64,
    min_harmony: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };

    match register_edge(&from, &to, weight, min_harmony) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_route_allowed(
    from: *const c_char,
    to: *const c_char,
    signal: f64,
    noise: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return 0;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return 0;
        }
    };

    match route_allowed(&from, &to, signal, noise) {
        Ok(true) => {
            clear_last_error();
            1
        }
        Ok(false) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_observe_route(
    from: *const c_char,
    to: *const c_char,
    success: i32,
    latency_ms: u64,
    cost_usd: f64,
) -> i32 {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };

    match observe_route(&from, &to, success != 0, latency_ms, cost_usd) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_log_event(
    component: *const c_char,
    direction: *const c_char,
    channel: *const c_char,
    payload: *const c_char,
    success: i32,
    error: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let direction = match cstr_to_string(direction) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };
    let channel = match cstr_to_string(channel) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return -1;
        }
    };

    let payload = cstr_to_optional_string(payload).unwrap_or_default();
    let error = cstr_to_optional_string(error).unwrap_or_default();

    match log_event(
        &component,
        &direction,
        &channel,
        &payload,
        success != 0,
        &error,
    ) {
        Ok(()) => {
            clear_last_error();
            0
        }
        Err(e) => {
            set_last_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_route_timeseries(
    from: *const c_char,
    to: *const c_char,
    limit: i32,
) -> *mut c_char {
    let from = match cstr_to_string(from) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return std::ptr::null_mut();
        }
    };
    let to = match cstr_to_string(to) {
        Ok(v) => v,
        Err(e) => {
            set_last_error(e);
            return std::ptr::null_mut();
        }
    };

    match route_timeseries(&from, &to, limit) {
        Ok(v) => {
            clear_last_error();
            to_c_string(v)
        }
        Err(e) => {
            set_last_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_time_report(since_unix: u64) -> *mut c_char {
    match time_report(since_unix) {
        Ok(v) => {
            clear_last_error();
            to_c_string(v)
        }
        Err(e) => {
            set_last_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_report() -> *mut c_char {
    match report() {
        Ok(v) => {
            clear_last_error();
            to_c_string(v)
        }
        Err(e) => {
            set_last_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_harmonic_matrix_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
