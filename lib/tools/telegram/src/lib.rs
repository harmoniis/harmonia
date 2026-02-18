use harmonia_vault::get_secret_for_symbol;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-telegram/0.1.0\0";
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}
fn set_error(m: impl Into<String>) {
    if let Ok(mut s) = last_error().write() {
        *s = m.into();
    }
}
fn clear_error() {
    if let Ok(mut s) = last_error().write() {
        s.clear();
    }
}
fn cstr_to_string(p: *const c_char) -> Result<String, String> {
    if p.is_null() {
        return Err("null pointer".into());
    };
    let c = unsafe { CStr::from_ptr(p) };
    Ok(c.to_string_lossy().into_owned())
}
#[no_mangle]
pub extern "C" fn harmonia_telegram_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_telegram_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_telegram_send_text(chat_id: *const c_char, text: *const c_char) -> i32 {
    let chat_id = match cstr_to_string(chat_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let text = match cstr_to_string(text) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let token = match get_secret_for_symbol("telegram_bot_token") {
        Some(v) => v,
        None => {
            set_error("missing secret: telegram_bot_token");
            return -1;
        }
    };
    let endpoint = env::var("HARMONIA_TELEGRAM_API_URL")
        .unwrap_or_else(|_| format!("https://api.telegram.org/bot{token}/sendMessage"));
    let payload = format!(
        "{{\"chat_id\":\"{}\",\"text\":\"{}\"}}",
        esc(&chat_id),
        esc(&text)
    );
    let out = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-d")
        .arg(payload)
        .arg(endpoint)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            clear_error();
            0
        }
        Ok(o) => {
            set_error(format!(
                "telegram send failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
            -1
        }
        Err(e) => {
            set_error(format!("curl exec failed: {e}"));
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
pub extern "C" fn harmonia_telegram_last_error() -> *mut c_char {
    CString::new(
        last_error()
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "telegram lock poisoned".into()),
    )
    .map(|s| s.into_raw())
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn harmonia_telegram_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
