use harmonia_vault::get_secret_for_symbol;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const VERSION: &[u8] = b"harmonia-elevenlabs/0.1.0\0";
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
pub extern "C" fn harmonia_elevenlabs_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_elevenlabs_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_elevenlabs_tts_to_file(
    text: *const c_char,
    voice_id: *const c_char,
    out_path: *const c_char,
) -> i32 {
    let text = match cs(text) {
        Ok(v) => v,
        Err(e) => {
            set(e);
            return -1;
        }
    };
    let voice = match cs(voice_id) {
        Ok(v) => v,
        Err(e) => {
            set(e);
            return -1;
        }
    };
    let out_path = match cs(out_path) {
        Ok(v) => v,
        Err(e) => {
            set(e);
            return -1;
        }
    };
    let key = match get_secret_for_symbol("elevenlabs_api_key") {
        Some(v) => v,
        None => {
            set("missing secret: elevenlabs_api_key");
            return -1;
        }
    };
    let endpoint = env::var("HARMONIA_ELEVENLABS_API_URL")
        .unwrap_or_else(|_| format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}"));
    let payload = format!(
        "{{\"text\":\"{}\",\"model_id\":\"eleven_multilingual_v2\"}}",
        esc(&text)
    );
    let out = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("xi-api-key: {key}"))
        .arg("-d")
        .arg(payload)
        .arg("-o")
        .arg(out_path)
        .arg(endpoint)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            clear();
            0
        }
        Ok(o) => {
            set(format!(
                "elevenlabs tts failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
            -1
        }
        Err(e) => {
            set(format!("curl exec failed: {e}"));
            -1
        }
    }
}
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
#[no_mangle]
pub extern "C" fn harmonia_elevenlabs_last_error() -> *mut c_char {
    to(le()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "elevenlabs lock poisoned".into()))
}
#[no_mangle]
pub extern "C" fn harmonia_elevenlabs_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
