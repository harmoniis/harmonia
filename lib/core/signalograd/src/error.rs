use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::Mutex;

use crate::checkpoint::load_state;
use crate::model::{KernelState, FEIGENBAUM_ALPHA, FEIGENBAUM_DELTA, LAST_ERROR, PHI, STATE};

pub(crate) fn last_error() -> &'static Mutex<String> {
    LAST_ERROR.get_or_init(|| Mutex::new(String::new()))
}

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
        .unwrap_or_else(|_| "signalograd error lock poisoned".to_string())
}

pub(crate) fn state() -> &'static Mutex<KernelState> {
    STATE.get_or_init(|| Mutex::new(load_state().unwrap_or_else(|_| KernelState::new())))
}

pub(crate) fn seeded_weight(a: usize, b: usize, scale: f64) -> f64 {
    let x = ((a + 1) as f64 * PHI + (b + 1) as f64 / FEIGENBAUM_DELTA).sin()
        + ((a + 1) as f64 / FEIGENBAUM_ALPHA + (b + 1) as f64 * 0.5).cos();
    clamp(x * 0.5 * scale, -scale, scale)
}

pub(crate) fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

pub(crate) fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub(crate) fn digest_hex(digest: u64) -> String {
    format!("{digest:016x}")
}

pub(crate) fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

pub(crate) fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
