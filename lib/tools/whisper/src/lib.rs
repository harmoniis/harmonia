use harmonia_vault::{get_secret_for_component, init_from_env};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const COMPONENT: &str = "whisper-tool";
const VERSION: &[u8] = b"harmonia-whisper/0.1.0\0";
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
fn le() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}
fn set(m: impl Into<String>) {
    if let Ok(mut s) = le().write() {
        *s = m.into();
    }
}
fn clear() {
    if let Ok(mut s) = le().write() {
        s.clear();
    }
}
fn cs(p: *const c_char) -> Result<String, String> {
    if p.is_null() {
        return Err("null pointer".into());
    };
    let c = unsafe { CStr::from_ptr(p) };
    Ok(c.to_string_lossy().into_owned())
}
fn to(v: String) -> *mut c_char {
    CString::new(v)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn harmonia_whisper_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_whisper_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_whisper_transcribe(audio_path: *const c_char) -> *mut c_char {
    let audio = match cs(audio_path) {
        Ok(v) => v,
        Err(e) => {
            set(e);
            return std::ptr::null_mut();
        }
    };
    let _ = init_from_env();
    let key = match get_secret_for_component("whisper-tool", "openai_api_key") {
        Ok(Some(v)) => v,
        Ok(None) => {
            set("missing secret: openai_api_key");
            return std::ptr::null_mut();
        }
        Err(e) => {
            set(format!("vault policy error: {e}"));
            return std::ptr::null_mut();
        }
    };
    let endpoint = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.openai.com/v1/audio/transcriptions".into());
    let model = harmonia_config_store::get_own_or(COMPONENT, "model", "whisper-1")
        .unwrap_or_else(|_| "whisper-1".into());
    let out = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg(format!("Authorization: Bearer {key}"))
        .arg("-F")
        .arg(format!("model={model}"))
        .arg("-F")
        .arg(format!("file=@{audio}"))
        .arg(endpoint)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            clear();
            to(String::from_utf8_lossy(&o.stdout).to_string())
        }
        Ok(o) => {
            set(format!(
                "whisper transcribe failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
            std::ptr::null_mut()
        }
        Err(e) => {
            set(format!("curl exec failed: {e}"));
            std::ptr::null_mut()
        }
    }
}
#[no_mangle]
pub extern "C" fn harmonia_whisper_last_error() -> *mut c_char {
    to(le()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "whisper lock poisoned".into()))
}
#[no_mangle]
pub extern "C" fn harmonia_whisper_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
