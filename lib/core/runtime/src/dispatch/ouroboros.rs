//! Ouroboros component dispatch — self-healing crash ledger and patch writing.
//!
//! Ops: healthcheck, record-crash, last-crash, history, write-patch.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};
use std::ffi::CString;

fn esc(s: &str) -> String {
    sexp_escape(s)
}

fn to_cstr(s: &str) -> *const std::os::raw::c_char {
    CString::new(s).map(|c| c.into_raw() as *const _).unwrap_or(std::ptr::null())
}

fn free_cstr(ptr: *mut std::os::raw::c_char) {
    if !ptr.is_null() {
        harmonia_ouroboros::harmonia_ouroboros_free_string(ptr);
    }
}

fn read_ffi_string(ptr: *mut std::os::raw::c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    free_cstr(ptr);
    s
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();

    match op.as_str() {
        "healthcheck" => {
            let ptr = harmonia_ouroboros::harmonia_ouroboros_health();
            let health = read_ffi_string(ptr);
            format!("(:ok :result \"{}\")", esc(&health))
        }

        "record-crash" => {
            let component = extract_sexp_string(sexp, ":component-name")
                .or_else(|| extract_sexp_string(sexp, ":component"))
                .unwrap_or_else(|| "unknown".into());
            let detail = extract_sexp_string(sexp, ":detail").unwrap_or_default();
            let comp_c = to_cstr(&component);
            let detail_c = to_cstr(&detail);
            let rc = harmonia_ouroboros::harmonia_ouroboros_record_crash(comp_c, detail_c);
            // Free the CStrings we allocated.
            if !comp_c.is_null() { unsafe { drop(CString::from_raw(comp_c as *mut _)); } }
            if !detail_c.is_null() { unsafe { drop(CString::from_raw(detail_c as *mut _)); } }
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                let err = read_ffi_string(harmonia_ouroboros::harmonia_ouroboros_last_error());
                format!("(:error \"record-crash: {}\")", esc(&err))
            }
        }

        "last-crash" => {
            let ptr = harmonia_ouroboros::harmonia_ouroboros_last_crash();
            let crash = read_ffi_string(ptr);
            format!("(:ok :result \"{}\")", esc(&crash))
        }

        "history" => {
            let limit = extract_sexp_string(sexp, ":limit")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(20);
            let ptr = harmonia_ouroboros::harmonia_ouroboros_history(limit);
            let history = read_ffi_string(ptr);
            format!("(:ok :result \"{}\")", esc(&history))
        }

        "write-patch" => {
            let component = extract_sexp_string(sexp, ":component-name")
                .or_else(|| extract_sexp_string(sexp, ":component"))
                .unwrap_or_else(|| "unknown".into());
            let body = extract_sexp_string(sexp, ":patch-body").unwrap_or_default();
            if body.is_empty() {
                return "(:error \"write-patch: :patch-body required\")".to_string();
            }
            let comp_c = to_cstr(&component);
            let body_c = to_cstr(&body);
            let rc = harmonia_ouroboros::harmonia_ouroboros_write_patch(comp_c, body_c);
            if !comp_c.is_null() { unsafe { drop(CString::from_raw(comp_c as *mut _)); } }
            if !body_c.is_null() { unsafe { drop(CString::from_raw(body_c as *mut _)); } }
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                let err = read_ffi_string(harmonia_ouroboros::harmonia_ouroboros_last_error());
                format!("(:error \"write-patch: {}\")", esc(&err))
            }
        }

        _ => format!("(:error \"ouroboros: unknown op '{}'\")", esc(&op)),
    }
}
