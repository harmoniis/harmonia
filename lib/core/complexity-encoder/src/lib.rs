//! Harmonia Complexity Encoder — 14-dimension prompt classifier for LLM routing.
//!
//! Zero-allocation hot path: operates on raw bytes with case-insensitive
//! matching. No `to_ascii_lowercase()`, no intermediate `Vec`, no `HashMap`.
//! All keyword sets are compile-time `&[&[u8]]` constants.
//!
//! Performance: ~6μs per classification in release, <2μs for short prompts.

mod dimensions;
#[cfg(test)]
mod integration_tests;
mod keywords;
pub mod scorer;
pub mod tier;

use std::ffi::{CStr, CString};
use std::fmt::Write;
use std::os::raw::c_char;

pub use scorer::score;
pub use tier::{ComplexityProfile, ComplexityTier};

const VERSION: &[u8] = b"harmonia-complexity-encoder/0.1.0\0";

// ── Public API ──────────────────────────────────────────────────────

/// Format a ComplexityProfile as an s-expression. Single allocation for output.
pub fn profile_to_sexp(profile: &ComplexityProfile) -> String {
    // Pre-calculate capacity: ~200 bytes for a typical sexp
    let mut out = String::with_capacity(256);
    out.push_str("(:tier \"");
    out.push_str(profile.tier.as_str());
    let _ = write!(
        out,
        "\" :score {:.4} :confidence {:.4} :dimensions (",
        profile.score, profile.confidence
    );
    for (i, d) in profile.dimensions.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let _ = write!(out, "{:.4}", d);
    }
    out.push_str("))");
    out
}

// ── FFI exports ─────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_complexity_encoder_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_complexity_encoder_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_complexity_encoder_score(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return std::ptr::null_mut();
    }
    let text = match unsafe { CStr::from_ptr(text) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let profile = score(text);
    let sexp = profile_to_sexp(&profile);
    match CString::new(sexp) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn harmonia_complexity_encoder_free_string(ptr: *mut c_char) {
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
    fn sexp_format() {
        let profile = score("hello");
        let sexp = profile_to_sexp(&profile);
        assert!(sexp.starts_with("(:tier "));
        assert!(sexp.contains(":score "));
        assert!(sexp.contains(":confidence "));
        assert!(sexp.contains(":dimensions ("));
    }

    #[test]
    fn ffi_healthcheck() {
        assert_eq!(harmonia_complexity_encoder_healthcheck(), 1);
    }

    #[test]
    fn ffi_null_input() {
        let result = harmonia_complexity_encoder_score(std::ptr::null());
        assert!(result.is_null());
    }

    #[test]
    fn ffi_roundtrip() {
        let input = CString::new("implement a distributed system").unwrap();
        let result = harmonia_complexity_encoder_score(input.as_ptr());
        assert!(!result.is_null());
        let output = unsafe { CStr::from_ptr(result) }.to_str().unwrap();
        assert!(output.contains(":tier"));
        harmonia_complexity_encoder_free_string(result);
    }
}
