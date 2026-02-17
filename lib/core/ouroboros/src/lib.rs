use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-ouroboros/0.2.0\0";

#[derive(Clone)]
struct CrashEvent {
    ts: u64,
    component: String,
    detail: String,
}

#[derive(Default)]
struct OuroborosState {
    crashes: Vec<CrashEvent>,
}

static STATE: OnceLock<RwLock<OuroborosState>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn state() -> &'static RwLock<OuroborosState> {
    STATE.get_or_init(|| RwLock::new(OuroborosState::default()))
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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
pub extern "C" fn harmonia_ouroboros_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_record_crash(
    component: *const c_char,
    detail: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
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
    let event = CrashEvent {
        ts: now_secs(),
        component,
        detail,
    };
    match state().write() {
        Ok(mut st) => {
            st.crashes.push(event);
            clear_error();
            0
        }
        Err(_) => {
            set_error("ouroboros lock poisoned");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_last_crash() -> *mut c_char {
    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("ouroboros lock poisoned");
            return std::ptr::null_mut();
        }
    };
    if let Some(last) = st.crashes.last() {
        clear_error();
        return to_c_string(format!(
            "(:ts {} :component {:?} :detail {:?})",
            last.ts, last.component, last.detail
        ));
    }
    set_error("no crash event");
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_write_patch(
    component: *const c_char,
    patch_body: *const c_char,
) -> i32 {
    let component = match cstr_to_string(component) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let patch = match cstr_to_string(patch_body) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    let patch_dir = env::var("HARMONIA_OUROBOROS_PATCH_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir().join("harmonia-ouroboros/patches"));
    if let Err(e) = fs::create_dir_all(&patch_dir) {
        set_error(format!("patch dir create failed: {e}"));
        return -1;
    }
    let filename = format!("{}-{}.patch", component.replace('/', "_"), now_secs());
    let path = patch_dir.join(filename);
    if let Err(e) = fs::write(&path, patch) {
        set_error(format!("patch write failed: {e}"));
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "ouroboros lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_ouroboros_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer must come from CString::into_raw in this crate.
    unsafe { drop(CString::from_raw(ptr)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_ouroboros_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_ouroboros_version().is_null());
    }

    #[test]
    fn record_and_read_crash() {
        let c = CString::new("http").unwrap();
        let d = CString::new("timeout panic").unwrap();
        assert_eq!(harmonia_ouroboros_record_crash(c.as_ptr(), d.as_ptr()), 0);
        let ptr = harmonia_ouroboros_last_crash();
        assert!(!ptr.is_null());
        let text = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().to_string();
        harmonia_ouroboros_free_string(ptr);
        assert!(text.contains("http"));
    }
}
