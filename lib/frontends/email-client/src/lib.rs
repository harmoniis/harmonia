use harmonia_vault::{get_secret_for_component, init_from_env};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const VERSION: &[u8] = b"harmonia-email-client/0.1.0\0";
const COMPONENT: &str = "email-frontend";
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
    let endpoint = harmonia_config_store::get_own_or(COMPONENT, "api-url", "https://api.resend.com/emails")
        .unwrap_or_else(|_| "https://api.resend.com/emails".into());
    let from = harmonia_config_store::get_own_or(COMPONENT, "from", "harmonia@local.invalid")
        .unwrap_or_else(|_| "harmonia@local.invalid".into());
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

// ---------------------------------------------------------------------------
// Gateway frontend contract wrappers (standard harmonia_frontend_* symbols)
// ---------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn harmonia_frontend_version() -> *const c_char {
    harmonia_email_client_version()
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_healthcheck() -> i32 {
    harmonia_email_client_healthcheck()
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_init(_config: *const c_char) -> i32 {
    cle();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        sete("poll: null buffer or zero length");
        return -1;
    }
    cle();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let subject =
        harmonia_config_store::get_own_or(COMPONENT, "default-subject", "Harmonia message").unwrap_or_else(|_| "Harmonia message".into());
    let c_subject = match CString::new(subject) {
        Ok(v) => v,
        Err(_) => {
            sete("invalid default email subject");
            return -1;
        }
    };
    harmonia_email_client_send(channel, c_subject.as_ptr(), payload)
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_last_error() -> *const c_char {
    harmonia_email_client_last_error() as *const c_char
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_shutdown() -> i32 {
    cle();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_free_string(ptr: *mut c_char) {
    harmonia_email_client_free_string(ptr)
}
