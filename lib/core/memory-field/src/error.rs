use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

use crate::model::{LAST_ERROR, STATE};
use crate::FieldState;

pub(crate) fn last_error() -> &'static Mutex<String> {
    LAST_ERROR.get_or_init(|| Mutex::new(String::new()))
}

#[allow(dead_code)]
pub(crate) fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().lock() {
        *slot = msg.into();
    }
}

pub(crate) fn clear_error() {
    if let Ok(mut slot) = last_error().lock() {
        slot.clear();
    }
}

pub(crate) fn last_error_message() -> String {
    last_error()
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_else(|_| "memory-field error lock poisoned".to_string())
}

pub(crate) fn state() -> &'static Mutex<FieldState> {
    STATE.get_or_init(|| Mutex::new(FieldState::new()))
}

pub(crate) fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

#[allow(dead_code)]
pub(crate) fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[allow(dead_code)]
pub(crate) fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

#[allow(dead_code)]
pub(crate) fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
