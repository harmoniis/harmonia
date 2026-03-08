use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::client;
use crate::state::{clear_error, last_error_message, set_error};

const VERSION: &[u8] = b"harmonia-openrouter/0.2.0\0";

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
pub extern "C" fn harmonia_openrouter_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_init() -> i32 {
    match client::init_backend() {
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
pub extern "C" fn harmonia_openrouter_complete(
    prompt: *const c_char,
    model: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(_) => String::new(),
    };

    match client::complete(&prompt, &model) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_free_string(ptr: *mut c_char) {
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

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_openrouter_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_openrouter_version().is_null());
    }
}
