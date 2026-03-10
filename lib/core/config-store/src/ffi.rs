use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::state::{clear_error, last_error_message, set_error};
use crate::{api, store};

const VERSION: &[u8] = b"harmonia-config-store/0.2.0\0";

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
    let value = c.to_string_lossy().trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ─── Simple FFI (no policy enforcement) ─────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_config_store_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_init() -> i32 {
    match api::init() {
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
pub extern "C" fn harmonia_config_store_set(
    scope: *const c_char,
    key: *const c_char,
    value: *const c_char,
) -> i32 {
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let value = match cstr_to_string(value) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match store::set_value(&scope, &key, &value) {
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
pub extern "C" fn harmonia_config_store_get(
    scope: *const c_char,
    key: *const c_char,
) -> *mut c_char {
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match store::get_value(&scope, &key) {
        Ok(Some(v)) => {
            clear_error();
            to_c_string(v)
        }
        Ok(None) => {
            clear_error();
            std::ptr::null_mut()
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_list(scope: *const c_char) -> *mut c_char {
    let scope = cstr_to_optional_string(scope);
    match store::list_keys(scope.as_deref()) {
        Ok(v) => {
            clear_error();
            to_c_string(v.join("\n"))
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

// ─── Component-aware FFI (new in 0.2.0) ─────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_config_store_get_for(
    component: *const c_char,
    scope: *const c_char,
    key: *const c_char,
) -> *mut c_char {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match api::get_config(&component, &scope, &key) {
        Ok(Some(v)) => {
            clear_error();
            to_c_string(v)
        }
        Ok(None) => {
            clear_error();
            std::ptr::null_mut()
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_get_or(
    component: *const c_char,
    scope: *const c_char,
    key: *const c_char,
    default: *const c_char,
) -> *mut c_char {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let default = match cstr_to_string(default) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match api::get_config_or(&component, &scope, &key, &default) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_set_for(
    component: *const c_char,
    scope: *const c_char,
    key: *const c_char,
    value: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let value = match cstr_to_string(value) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match api::set_config(&component, &scope, &key, &value) {
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
pub extern "C" fn harmonia_config_store_delete_for(
    component: *const c_char,
    scope: *const c_char,
    key: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let key = match cstr_to_string(key) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match api::delete_config(&component, &scope, &key) {
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
pub extern "C" fn harmonia_config_store_dump(
    component: *const c_char,
    scope: *const c_char,
) -> *mut c_char {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let scope = match cstr_to_string(scope) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    match api::dump_scope(&component, &scope) {
        Ok(pairs) => {
            clear_error();
            let text: String = pairs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("\n");
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_config_store_ingest_env() -> i32 {
    match crate::ingest::seed_from_env() {
        Ok(_) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_config_store_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_non_null() {
        assert!(!harmonia_config_store_version().is_null());
    }
}
