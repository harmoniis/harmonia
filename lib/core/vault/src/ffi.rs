use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::api::{
    has_secret_for_symbol, init_from_env, list_secret_symbols, set_secret_for_symbol,
};
use crate::state::{clear_error, last_error, set_error};

const VERSION: &[u8] = b"harmonia-vault/0.3.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }

    // Safety: caller must provide valid null-terminated C string.
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
pub extern "C" fn harmonia_vault_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_vault_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_vault_init() -> i32 {
    match init_from_env() {
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
pub extern "C" fn harmonia_vault_get_secret(_symbol: *const c_char) -> *mut c_char {
    set_error("vault read denied over C API; use backend-internal vault access");
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn harmonia_vault_has_secret(symbol: *const c_char) -> i32 {
    let symbol = match cstr_to_string(symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    if has_secret_for_symbol(&symbol) {
        clear_error();
        1
    } else {
        clear_error();
        0
    }
}

#[no_mangle]
pub extern "C" fn harmonia_vault_list_symbols() -> *mut c_char {
    let list = list_secret_symbols();
    clear_error();
    to_c_string(list.join("\n"))
}

#[no_mangle]
pub extern "C" fn harmonia_vault_set_secret(symbol: *const c_char, value: *const c_char) -> i32 {
    let symbol = match cstr_to_string(symbol) {
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
    match set_secret_for_symbol(&symbol, &value) {
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
pub extern "C" fn harmonia_vault_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "vault lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_vault_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // Safety: ptr must come from CString::into_raw from this crate.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_vault_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_vault_version().is_null());
    }
}
