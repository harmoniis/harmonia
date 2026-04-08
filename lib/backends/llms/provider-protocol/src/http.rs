//! HTTP helpers for LLM provider backends.

use serde_json::Value;
use std::process::Command;

/// Timeout configuration for HTTP calls.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub connect_timeout_secs: u64,
    pub max_time_secs: u64,
}

fn parse_u64(raw: Option<String>) -> Option<u64> {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
}

/// Read timeout configuration from config-store (component scope), then legacy env fallback.
/// E.g. component `openai-backend` uses keys `connect-timeout-secs` and `max-time-secs`.
pub fn get_timeout(
    component: &str,
    _legacy_env_prefix: &str,
    connect_default: u64,
    max_default: u64,
) -> TimeoutConfig {
    TimeoutConfig {
        connect_timeout_secs: parse_u64(
            harmonia_config_store::get_own(component, "connect-timeout-secs")
                .ok()
                .flatten(),
        )
        .unwrap_or(connect_default),
        max_time_secs: parse_u64(
            harmonia_config_store::get_own(component, "max-time-secs")
                .ok()
                .flatten(),
        )
        .unwrap_or(max_default),
    }
}

/// Truncate text to `limit` chars, appending `"..."` if truncated.
pub fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push_str("...");
    out
}

/// Parse a JSON response body, returning an error if empty or malformed.
pub fn parse_json_response(stdout: &str) -> Result<Value, String> {
    if stdout.trim().is_empty() {
        return Err("empty response".to_string());
    }
    serde_json::from_str(stdout)
        .map_err(|e| format!("invalid JSON response: {e}; body={}", clip(stdout, 320)))
}

/// Check a parsed JSON response for provider error messages.
pub fn json_error_message(v: &Value) -> Option<String> {
    if let Some(msg) = v
        .get("error")
        .and_then(|x| x.get("message").or_else(|| x.get("msg")))
        .and_then(|x| x.as_str())
    {
        return Some(msg.to_string());
    }
    if let Some(msg) = v.get("message").and_then(|x| x.as_str()) {
        let lowered = msg.to_ascii_lowercase();
        if lowered.contains("error") || lowered.contains("invalid") || lowered.contains("denied") {
            return Some(msg.to_string());
        }
    }
    None
}

/// Execute an HTTP POST with JSON body via `curl`, return parsed JSON.
/// Returns structured CompletionError on failure for circuit-breaker + signalograd feedback.
pub fn run_curl_json_post(
    url: &str,
    headers: &[String],
    payload: &Value,
    timeout: TimeoutConfig,
) -> Result<Value, String> {
    let provider = url.split('/').nth(2).unwrap_or("unknown").to_string();
    let payload_str =
        serde_json::to_string(payload).map_err(|e| format!("json encode failed: {e}"))?;
    let args: Vec<String> = [
        "-sS", "-X", "POST",
        "--connect-timeout", &timeout.connect_timeout_secs.to_string(),
        "--max-time", &timeout.max_time_secs.to_string(),
        url, "-H", "Content-Type: application/json",
    ].iter().map(|s| s.to_string())
     .chain(headers.iter().flat_map(|h| vec!["-H".to_string(), h.clone()]))
     .chain(std::iter::once("-d".to_string()))
     .chain(std::iter::once(payload_str))
     .collect();
    let output = Command::new("curl").args(&args).output()
        .map_err(|e| crate::error::CompletionError::ConnectionFailed {
            provider: provider.clone(), detail: format!("curl exec: {e}"),
        }.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = clip(&stderr, 320);
        // Classify curl exit codes: 7=connection refused, 28=timeout, 35=TLS
        let code = output.status.code().unwrap_or(-1);
        return Err(match code {
            7 | 35 => crate::error::CompletionError::ConnectionFailed {
                provider, detail }.to_string(),
            28 => crate::error::CompletionError::Timeout {
                provider, elapsed_ms: timeout.max_time_secs * 1000 }.to_string(),
            _ => crate::error::CompletionError::ServerError {
                provider, status: 0, detail }.to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed = parse_json_response(&stdout)
        .map_err(|e| crate::error::CompletionError::ParseError {
            provider: provider.clone(), detail: e }.to_string())?;
    if let Some(msg) = json_error_message(&parsed) {
        return Err(crate::error::CompletionError::ModelError {
            provider, model: String::new(), detail: msg }.to_string());
    }
    Ok(parsed)
}

/// Read a secret from the vault, trying multiple symbols, returning the first non-empty.
pub fn get_secret_any(component: &str, symbols: &[&str]) -> Result<Option<String>, String> {
    for symbol in symbols {
        let got = harmonia_vault::get_secret_for_component(component, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(v) = got {
            if !v.trim().is_empty() {
                return Ok(Some(v));
            }
        }
    }
    Ok(None)
}
