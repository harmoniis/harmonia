use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use crate::backend;
use harmonia_voice_protocol::{clear_error, last_error_message, set_error};

const VERSION: &[u8] = b"harmonia-elevenlabs/0.2.0\0";

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

pub fn harmonia_elevenlabs_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_elevenlabs_healthcheck() -> i32 {
    1
}

pub fn harmonia_elevenlabs_init() -> i32 {
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

pub fn harmonia_elevenlabs_tts_to_file(
    text: *const c_char,
    voice_id: *const c_char,
    out_path: *const c_char,
) -> i32 {
    let text = match cstr_to_string(text) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let voice = match cstr_to_string(voice_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let out = match cstr_to_string(out_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match backend::tts_to_file(&text, &voice, &out, "") {
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

pub fn harmonia_elevenlabs_tts_to_file_with_model(
    text: *const c_char,
    voice_id: *const c_char,
    out_path: *const c_char,
    model: *const c_char,
) -> i32 {
    let text = match cstr_to_string(text) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let voice = match cstr_to_string(voice_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let out = match cstr_to_string(out_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let model = cstr_to_string(model).unwrap_or_default();
    match backend::tts_to_file(&text, &voice, &out, &model) {
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

pub fn harmonia_elevenlabs_list_voices() -> *mut c_char {
    match backend::list_voices() {
        Ok(json) => {
            clear_error();
            to_c(json)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

pub fn harmonia_elevenlabs_list_models() -> *mut c_char {
    to_c(backend::list_offerings())
}

pub fn harmonia_elevenlabs_last_error() -> *mut c_char {
    to_c(last_error_message())
}

pub fn harmonia_elevenlabs_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}
