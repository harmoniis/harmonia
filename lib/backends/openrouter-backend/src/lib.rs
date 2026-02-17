use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::Command;
use std::env;
use std::sync::{OnceLock, RwLock};

use harmonia_vault::{get_secret_for_symbol, init_from_env};

const VERSION: &[u8] = b"harmonia-openrouter/0.2.0\0";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

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

    // Safety: caller provides valid null-terminated pointer.
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn extract_content_from_response(payload: &str) -> Option<String> {
    let key = "\"content\":\"";
    let start = payload.find(key)? + key.len();
    let rest = &payload[start..];
    let mut escaped = false;
    let mut out = String::new();
    for ch in rest.chars() {
        if escaped {
            let decoded = match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '"' => '"',
                '\\' => '\\',
                other => other,
            };
            out.push(decoded);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return Some(out);
        }
        out.push(ch);
    }
    None
}

fn extract_error_message(payload: &str) -> Option<String> {
    if !payload.contains("\"error\"") {
        return None;
    }
    let key = "\"message\":\"";
    let start = payload.find(key)?;
    let rest = &payload[start + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn request_once(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let payload = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
        json_escape(model),
        json_escape(prompt)
    );

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg(OPENROUTER_URL)
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg("HTTP-Referer: https://harmoniis.local")
        .arg("-H")
        .arg("X-Title: Harmonia Agent")
        .arg("-d")
        .arg(payload)
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("curl failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    if stdout.trim().is_empty() {
        return Err("openrouter empty response".to_string());
    }
    if let Some(err) = extract_error_message(&stdout) {
        return Err(err);
    }

    if let Some(content) = extract_content_from_response(&stdout) {
        return Ok(content);
    }

    Err(format!("missing content in response: {stdout}"))
}

fn fallback_models() -> Vec<String> {
    let raw = env::var("HARMONIA_OPENROUTER_FALLBACK_MODELS").unwrap_or_else(|_| {
        "google/gemma-3-4b-it:free,qwen/qwen3-next-80b-a3b-instruct:free,meta-llama/llama-3.2-3b-instruct:free"
            .to_string()
    });
    raw.split(',')
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect()
}

fn openrouter_complete(prompt: &str, model: &str) -> Result<String, String> {
    let api_key = get_secret_for_symbol("openrouter")
        .ok_or_else(|| "openrouter key missing in vault".to_string())?;

    match request_once(prompt, model, &api_key) {
        Ok(text) => Ok(text),
        Err(primary_err) => {
            for fallback in fallback_models() {
                if fallback == model {
                    continue;
                }
                if let Ok(text) = request_once(prompt, &fallback, &api_key) {
                    return Ok(text);
                }
            }
            Err(primary_err)
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_init() -> i32 {
    match init_from_env() {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_complete(
    prompt: *const c_char,
    model: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };

    let model = match cstr_to_string(model) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => "qwen/qwen3-coder:free".to_string(),
    };

    match openrouter_complete(&prompt, &model) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "openrouter lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_openrouter_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }

    // Safety: ptr must come from CString::into_raw in this crate.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_openrouter_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_openrouter_version().is_null());
    }
}
