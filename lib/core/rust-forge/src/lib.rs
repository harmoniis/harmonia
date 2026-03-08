use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-rust-forge/0.2.0\0";

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

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_build_package(
    workspace_dir: *const c_char,
    package: *const c_char,
) -> i32 {
    let workspace_dir = match cstr_to_string(workspace_dir) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let package = match cstr_to_string(package) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let output = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg(package)
        .current_dir(workspace_dir)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            clear_error();
            0
        }
        Ok(out) => {
            set_error(format!(
                "forge build failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
            -1
        }
        Err(e) => {
            set_error(format!("forge build exec failed: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "forge lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_rust_forge_free_string(ptr: *mut c_char) {
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
        assert_eq!(harmonia_rust_forge_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_rust_forge_version().is_null());
    }
}
