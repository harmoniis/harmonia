use harmonia_vault::{get_secret_for_component, init_from_env};
use std::env;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const VERSION: &[u8] = b"harmonia-email-client/0.1.0\0";
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
pub extern "C" fn harmonia_email_client_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_email_client_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_email_client_send(
    to: *const c_char,
    subject: *const c_char,
    body: *const c_char,
) -> i32 {
    let to = match cs(to) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let subject = match cs(subject) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let body = match cs(body) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let _ = init_from_env();
    let token = match get_secret_for_component("email-frontend", "email_api_key") {
        Ok(Some(v)) => v,
        Ok(None) => {
            sete("missing secret: email_api_key");
            return -1;
        }
        Err(e) => {
            sete(format!("vault policy error: {e}"));
            return -1;
        }
    };
    let endpoint = env::var("HARMONIA_EMAIL_API_URL")
        .unwrap_or_else(|_| "https://api.resend.com/emails".into());
    let from = env::var("HARMONIA_EMAIL_FROM").unwrap_or_else(|_| "harmonia@local.invalid".into());
    let payload = format!(
        "{{\"from\":\"{}\",\"to\":[\"{}\"],\"subject\":\"{}\",\"text\":\"{}\"}}",
        esc(&from),
        esc(&to),
        esc(&subject),
        esc(&body)
    );
    let out = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("Authorization: Bearer {token}"))
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
                "email send failed: {}",
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
pub extern "C" fn harmonia_email_client_last_error() -> *mut c_char {
    CString::new(
        le().read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "email lock poisoned".into()),
    )
    .map(|s| s.into_raw())
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn harmonia_email_client_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
