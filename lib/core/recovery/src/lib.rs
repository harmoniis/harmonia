use std::env;
use std::ffi::{CStr, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-recovery/0.2.0\0";

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    // Safety: caller provides valid null-terminated string.
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn state_root() -> String {
    env::var("HARMONIA_STATE_ROOT").unwrap_or_else(|_| {
        env::temp_dir()
            .join("harmonia")
            .to_string_lossy()
            .to_string()
    })
}

fn log_path() -> String {
    env::var("HARMONIA_RECOVERY_LOG").unwrap_or_else(|_| format!("{}/recovery.log", state_root()))
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_record(kind: *const c_char, detail: *const c_char) -> i32 {
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let detail = match cstr_to_string(detail) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let path = log_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            set_error(format!("create recovery dir failed: {e}"));
            return -1;
        }
    }
    let mut file = match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("open recovery log failed: {e}"));
            return -1;
        }
    };
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Err(e) = writeln!(file, "{}\t{}\t{}", ts, kind, detail) {
        set_error(format!("write recovery log failed: {e}"));
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_tail_lines(limit: i32) -> *mut c_char {
    let path = log_path();
    let body = match fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("read recovery log failed: {e}"));
            return std::ptr::null_mut();
        }
    };
    let n = if limit <= 0 { 20 } else { limit as usize };
    let lines: Vec<&str> = body.lines().collect();
    let start = lines.len().saturating_sub(n);
    clear_error();
    to_c_string(lines[start..].join("\n"))
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "recovery lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_recovery_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer comes from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_recovery_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_recovery_version().is_null());
    }
}
