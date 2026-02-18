use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::{Component, Path, PathBuf};
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-fs/0.2.0\0";

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

fn sandbox_root() -> PathBuf {
    env::var("HARMONIA_FS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/harmonia-fs"))
}

fn resolve_in_sandbox(input: &str) -> Result<PathBuf, String> {
    let root = sandbox_root();
    let input_path = Path::new(input);
    if input_path.is_absolute() {
        return Err("absolute paths are not allowed".to_string());
    }
    for c in input_path.components() {
        if matches!(c, Component::ParentDir) {
            return Err("path traversal rejected".to_string());
        }
    }
    let full = root.join(input_path);
    let full_norm = full.components().collect::<PathBuf>();
    if !full_norm.starts_with(&root) {
        return Err("path traversal rejected".to_string());
    }
    Ok(full_norm)
}

#[no_mangle]
pub extern "C" fn harmonia_fs_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_fs_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_fs_write(path: *const c_char, content: *const c_char) -> i32 {
    let path = match cstr_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let content = match cstr_to_string(content) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let full = match resolve_in_sandbox(&path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    if let Some(parent) = full.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            set_error(format!("create dir failed: {e}"));
            return -1;
        }
    }
    match fs::write(full, content) {
        Ok(_) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(format!("write failed: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_fs_read(path: *const c_char) -> *mut c_char {
    let path = match cstr_to_string(path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let full = match resolve_in_sandbox(&path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    match fs::read_to_string(full) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(format!("read failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_fs_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "fs lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_fs_free_string(ptr: *mut c_char) {
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
        assert_eq!(harmonia_fs_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_fs_version().is_null());
    }
}
