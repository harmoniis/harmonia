use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::backend;
use harmonia_provider_protocol::{clear_error, last_error_message, set_error};

const VERSION: &[u8] = b"harmonia-alibaba/0.2.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    Ok(unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned())
}

fn to_c(value: String) -> *mut c_char {
    CString::new(value)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

pub fn harmonia_alibaba_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
pub fn harmonia_alibaba_healthcheck() -> i32 {
    1
}

pub fn harmonia_alibaba_init() -> i32 {
    match backend::init() {
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

pub fn harmonia_alibaba_complete(prompt: *const c_char, model: *const c_char) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let model = cstr_to_string(model).unwrap_or_default();
    match backend::complete(&prompt, &model) {
        Ok(t) => {
            clear_error();
            to_c(t)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

pub fn harmonia_alibaba_list_models() -> *mut c_char {
    to_c(backend::list_offerings())
}

pub fn harmonia_alibaba_select_model(hint: *const c_char) -> *mut c_char {
    to_c(backend::select_model(
        &cstr_to_string(hint).unwrap_or_default(),
    ))
}

pub fn harmonia_alibaba_complete_for_task(
    prompt: *const c_char,
    hint: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let hint = cstr_to_string(hint).unwrap_or_default();
    match backend::complete_for_task(&prompt, &hint) {
        Ok(t) => {
            clear_error();
            to_c(t)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

pub fn harmonia_alibaba_last_error() -> *mut c_char {
    to_c(last_error_message())
}
pub fn harmonia_alibaba_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}
