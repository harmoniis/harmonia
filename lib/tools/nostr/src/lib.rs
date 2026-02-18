use harmonia_vault::get_secret_for_symbol;
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const VERSION: &[u8] = b"harmonia-nostr/0.1.0\0";
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
fn le() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}
fn sete(m: impl Into<String>) {
    if let Ok(mut s) = le().write() {
        *s = m.into();
    }
}
fn cle() {
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
#[no_mangle]
pub extern "C" fn harmonia_nostr_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_nostr_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_nostr_publish_text(text: *const c_char) -> i32 {
    let text = match cs(text) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let sk = match get_secret_for_symbol("nostr_private_key") {
        Some(v) => v,
        None => {
            sete("missing secret: nostr_private_key");
            return -1;
        }
    };
    let endpoint = env::var("HARMONIA_NOSTR_API_URL")
        .unwrap_or_else(|_| "https://nostr.example/publish".into());
    let payload = format!(
        "{{\"content\":\"{}\",\"private_key\":\"{}\"}}",
        esc(&text),
        esc(&sk)
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
            cle();
            0
        }
        Ok(o) => {
            sete(format!(
                "nostr publish failed: {}",
                String::from_utf8_lossy(&o.stderr)
            ));
            -1
        }
        Err(e) => {
            sete(format!("curl exec failed: {e}"));
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
pub extern "C" fn harmonia_nostr_last_error() -> *mut c_char {
    CString::new(
        le().read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "nostr lock poisoned".into()),
    )
    .map(|s| s.into_raw())
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn harmonia_nostr_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
