//! HTTP fetch logic with timeout and size limits.

use crate::sandbox;
use harmonia_vault::{get_secret_for_component, init_from_env};
use std::io::Read as _;
use std::time::Duration;

use super::config;

/// Fetch a URL via HTTP GET, enforcing timeout and size limits.
pub fn fetch(url: &str) -> Result<String, String> {
    let (ua, timeout_ms, max_bytes, allowlist) = {
        let st = config::state()
            .read()
            .map_err(|e| format!("lock poisoned: {e}"))?;
        (
            st.user_agent.clone(),
            st.timeout_ms,
            st.max_response_bytes,
            st.network_allowlist.clone(),
        )
    };

    sandbox::check_domain_allowed(url, &allowlist)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(timeout_ms))
        .timeout_read(Duration::from_millis(timeout_ms))
        .user_agent(&ua)
        .build();

    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    read_response(response, max_bytes)
}

/// Fetch a URL with vault-injected Bearer token authentication.
pub fn fetch_with_auth(url: &str, auth_symbol: &str) -> Result<String, String> {
    let (ua, timeout_ms, max_bytes, allowlist) = {
        let st = config::state()
            .read()
            .map_err(|e| format!("lock poisoned: {e}"))?;
        (
            st.user_agent.clone(),
            st.timeout_ms,
            st.max_response_bytes,
            st.network_allowlist.clone(),
        )
    };

    sandbox::check_domain_allowed(url, &allowlist)?;

    let _ = init_from_env();
    let secret = get_secret_for_component("browser-tool", auth_symbol)
        .map_err(|e| format!("vault policy error: {e}"))?
        .ok_or_else(|| format!("missing secret for symbol: {}", auth_symbol))?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(timeout_ms))
        .timeout_read(Duration::from_millis(timeout_ms))
        .user_agent(&ua)
        .build();

    let response = agent
        .get(url)
        .set("Authorization", &format!("Bearer {}", secret))
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    read_response(response, max_bytes)
}

/// Read an HTTP response body with size limit enforcement.
fn read_response(response: ureq::Response, max_bytes: usize) -> Result<String, String> {
    if let Some(len_str) = response.header("content-length") {
        if let Ok(len) = len_str.parse::<usize>() {
            if len > max_bytes {
                return Err(format!(
                    "response too large: {} bytes (limit: {})",
                    len, max_bytes
                ));
            }
        }
    }

    let mut buf = Vec::with_capacity(max_bytes.min(65536));
    let mut reader = response.into_reader();
    let mut total = 0usize;
    loop {
        let mut chunk = [0u8; 8192];
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read error: {e}"))?;
        if n == 0 {
            break;
        }
        total += n;
        if total > max_bytes {
            return Err(format!("response too large (>{} bytes)", max_bytes));
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}
