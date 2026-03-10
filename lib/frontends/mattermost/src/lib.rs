use harmonia_vault::{get_secret_for_component, init_from_env};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
const VERSION: &[u8] = b"harmonia-mattermost/0.1.0\0";
const COMPONENT: &str = "mattermost-frontend";
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
pub extern "C" fn harmonia_mattermost_version() -> *const c_char {
    VERSION.as_ptr().cast()
}
#[no_mangle]
pub extern "C" fn harmonia_mattermost_healthcheck() -> i32 {
    1
}
#[no_mangle]
pub extern "C" fn harmonia_mattermost_send_text(
    channel: *const c_char,
    text: *const c_char,
) -> i32 {
    let channel = match cs(channel) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let text = match cs(text) {
        Ok(v) => v,
        Err(e) => {
            sete(e);
            return -1;
        }
    };
    let _ = init_from_env();
    let token = match get_secret_for_component("mattermost-frontend", "mattermost_bot_token") {
        Ok(Some(v)) => v,
        Ok(None) => {
            sete("missing secret: mattermost_bot_token");
            return -1;
        }
        Err(e) => {
            sete(format!("vault policy error: {e}"));
            return -1;
        }
    };
    let endpoint = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://mattermost.example/api/v4/posts".into());
    let payload = format!(
        "{{\"channel_id\":\"{}\",\"message\":\"{}\"}}",
        esc(&channel),
        esc(&text)
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
                "mattermost send failed: {}",
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
pub extern "C" fn harmonia_mattermost_last_error() -> *mut c_char {
    CString::new(
        le().read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "mattermost lock poisoned".into()),
    )
    .map(|s| s.into_raw())
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn harmonia_mattermost_free_string(ptr: *mut c_char) {
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
    harmonia_mattermost_version()
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_healthcheck() -> i32 {
    harmonia_mattermost_healthcheck()
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
    harmonia_mattermost_send_text(channel, payload)
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_last_error() -> *const c_char {
    harmonia_mattermost_last_error() as *const c_char
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_shutdown() -> i32 {
    cle();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_free_string(ptr: *mut c_char) {
    harmonia_mattermost_free_string(ptr)
}
