use std::env;
use std::ffi::{CStr, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{OnceLock, RwLock};

const VERSION: &[u8] = b"harmonia-push-sns/0.2.0\0";

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

fn state_root() -> String {
    env::var("HARMONIA_STATE_ROOT").unwrap_or_else(|_| {
        env::temp_dir()
            .join("harmonia")
            .to_string_lossy()
            .to_string()
    })
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_send(
    topic_arn: *const c_char,
    subject: *const c_char,
    message: *const c_char,
) -> i32 {
    let topic_arn = match cstr_to_string(topic_arn) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let subject = match cstr_to_string(subject) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let message = match cstr_to_string(message) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    if env::var("HARMONIA_PUSH_SNS_MODE")
        .map(|v| v.eq_ignore_ascii_case("log"))
        .unwrap_or(false)
    {
        let path = env::var("HARMONIA_PUSH_SNS_LOG")
            .unwrap_or_else(|_| format!("{}/push.log", state_root()));
        if let Some(parent) = std::path::Path::new(&path).parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                set_error(format!("push log dir create failed: {e}"));
                return -1;
            }
        }
        match OpenOptions::new().create(true).append(true).open(path) {
            Ok(mut f) => {
                if let Err(e) = writeln!(f, "{}\t{}\t{}", topic_arn, subject, message) {
                    set_error(format!("push log write failed: {e}"));
                    return -1;
                }
                clear_error();
                return 0;
            }
            Err(e) => {
                set_error(format!("push log open failed: {e}"));
                return -1;
            }
        }
    }

    let output = Command::new("aws")
        .arg("sns")
        .arg("publish")
        .arg("--topic-arn")
        .arg(topic_arn)
        .arg("--subject")
        .arg(subject)
        .arg("--message")
        .arg(message)
        .output();
    match output {
        Ok(out) if out.status.success() => {
            clear_error();
            0
        }
        Ok(out) => {
            set_error(format!(
                "aws sns publish failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
            -1
        }
        Err(e) => {
            set_error(format!("aws sns exec failed: {e}"));
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "push-sns lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_push_sns_free_string(ptr: *mut c_char) {
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
        assert_eq!(harmonia_push_sns_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_push_sns_version().is_null());
    }
}
