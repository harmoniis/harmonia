use std::env;
use std::process::Command;

use harmonia_config_store::get_value as get_config_value;
use harmonia_vault::{get_secret_for_symbol, init_from_env};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

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
    let connect_timeout = env::var("HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10);
    let max_time = env::var("HARMONIA_OPENROUTER_MAX_TIME_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(45);

    let payload = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
        json_escape(model),
        json_escape(prompt)
    );

    let output = Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("--connect-timeout")
        .arg(connect_timeout.to_string())
        .arg("--max-time")
        .arg(max_time.to_string())
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

fn configured_value(key: &str) -> Option<String> {
    get_config_value("global", key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn fallback_models() -> Vec<String> {
    let raw = configured_value("openrouter.fallback_models")
        .or_else(|| env::var("HARMONIA_OPENROUTER_FALLBACK_MODELS").ok())
        .unwrap_or_default();
    raw.split(',')
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect()
}

fn default_model() -> Result<String, String> {
    if let Some(v) = configured_value("openrouter.default_model") {
        return Ok(v);
    }
    if let Some(v) = configured_value("model.default") {
        return Ok(v);
    }
    if let Some(first) = fallback_models().into_iter().next() {
        return Ok(first);
    }
    Err("no openrouter default model configured".to_string())
}

pub(crate) fn init_backend() -> Result<(), String> {
    init_from_env()
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init_from_env();
    let api_key = get_secret_for_symbol("openrouter")
        .ok_or_else(|| "openrouter key missing in vault".to_string())?;
    let selected_model = if model.trim().is_empty() {
        default_model()?
    } else {
        model.to_string()
    };

    match request_once(prompt, &selected_model, &api_key) {
        Ok(text) => Ok(text),
        Err(primary_err) => {
            for fallback in fallback_models() {
                if fallback == selected_model {
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
