//! Controlled HTTP fetch — secure proxy for agent JS code API calls.
//!
//! When agent code (or future V8 sandbox code) needs to make HTTP requests,
//! it MUST go through this module instead of making direct network calls.
//! This provides:
//!
//! - Domain allowlist enforcement (same as browser sandbox)
//! - Dangerous target blocking (localhost, metadata endpoints, internal IPs)
//! - Response size limits
//! - Timeout enforcement
//! - Security boundary wrapping on all responses
//!
//! Architecture: AgentBrowser.fetch() → Rust/ureq → target
//! The JS code never gets direct network access.

use crate::sandbox;
use crate::security;
use serde_json::json;
use std::io::Read as _;
use std::time::Duration;

/// Targets that are ALWAYS blocked regardless of allowlist.
/// These prevent SSRF attacks against internal services.
const DANGEROUS_TARGETS: &[&str] = &[
    // Localhost variants
    "localhost",
    "127.0.0.1",
    "0.0.0.0",
    "[::1]",
    "0177.0.0.1", // Octal localhost
    "2130706433", // Decimal localhost
    // Link-local (AWS/GCP/Azure metadata)
    "169.254.169.254",
    "metadata.google.internal",
    // Internal network ranges (checked by prefix in is_dangerous_host)
    // 10.x.x.x, 172.16-31.x.x, 192.168.x.x handled separately
];

/// Internal IP prefixes that are blocked.
const INTERNAL_IP_PREFIXES: &[&str] = &[
    "10.", "172.16.", "172.17.", "172.18.", "172.19.", "172.20.", "172.21.", "172.22.", "172.23.",
    "172.24.", "172.25.", "172.26.", "172.27.", "172.28.", "172.29.", "172.30.", "172.31.",
    "192.168.", "fc00:", "fd00:", // IPv6 ULA
    "fe80:", // IPv6 link-local
];

/// Configuration for controlled fetch operations.
pub struct ControlledFetchConfig {
    /// Maximum response body size in bytes.
    pub max_response_bytes: usize,
    /// Request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Domain allowlist. None = allow all (except dangerous).
    pub domain_allowlist: Option<Vec<String>>,
    /// User-Agent string for requests.
    pub user_agent: String,
}

impl Default for ControlledFetchConfig {
    fn default() -> Self {
        Self {
            max_response_bytes: 2 * 1024 * 1024, // 2 MB
            timeout_ms: 10_000,
            domain_allowlist: None,
            user_agent: "harmonia-browser/2.0.0".to_string(),
        }
    }
}

/// Check if a hostname is a dangerous target (internal/metadata/localhost).
pub fn is_dangerous_host(host: &str) -> bool {
    let lower = host.to_lowercase();

    // Check exact matches
    for target in DANGEROUS_TARGETS {
        if lower == *target {
            return true;
        }
    }

    // Check internal IP prefixes
    for prefix in INTERNAL_IP_PREFIXES {
        if lower.starts_with(prefix) {
            return true;
        }
    }

    // Block any IP that resolves to 127.x.x.x
    if lower.starts_with("127.") {
        return true;
    }

    false
}

/// Extract host from a URL string.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };

    let after_user = if let Some(pos) = after_scheme.find('@') {
        &after_scheme[pos + 1..]
    } else {
        after_scheme
    };

    let end = after_user
        .find(|c: char| c == '/' || c == ':' || c == '?' || c == '#')
        .unwrap_or(after_user.len());

    let host = &after_user[..end];
    if host.is_empty() {
        None
    } else {
        Some(host.to_lowercase())
    }
}

/// Perform a controlled HTTP GET request.
///
/// Enforces:
/// 1. Dangerous target blocking (SSRF prevention)
/// 2. Domain allowlist (if configured)
/// 3. Response size limits
/// 4. Timeout
///
/// Returns the response body as a string, wrapped in security boundary.
pub fn controlled_get(url: &str, config: &ControlledFetchConfig) -> Result<String, String> {
    // 1. Extract and validate host
    let host = extract_host(url).ok_or_else(|| format!("cannot parse host from URL: {}", url))?;

    // 2. Block dangerous targets (ALWAYS, regardless of allowlist)
    if is_dangerous_host(&host) {
        return Err(format!(
            "BLOCKED: dangerous target '{}' — internal/metadata endpoints are never allowed",
            host
        ));
    }

    // 3. Check domain allowlist
    sandbox::check_domain_allowed(url, &config.domain_allowlist)?;

    // 4. Perform the fetch with timeout and size limits
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(config.timeout_ms))
        .timeout_read(Duration::from_millis(config.timeout_ms))
        .user_agent(&config.user_agent)
        .redirects(5) // Follow redirects but limit depth
        .build();

    let response = agent
        .get(url)
        .call()
        .map_err(|e| format!("controlled_fetch failed: {}", e))?;

    // Check content-length
    if let Some(len_str) = response.header("content-length") {
        if let Ok(len) = len_str.parse::<usize>() {
            if len > config.max_response_bytes {
                return Err(format!(
                    "response too large: {} bytes (limit: {})",
                    len, config.max_response_bytes
                ));
            }
        }
    }

    // Read with size limit
    let mut buf = Vec::with_capacity(config.max_response_bytes.min(65536));
    let mut reader = response.into_reader();
    let mut total = 0usize;
    loop {
        let mut chunk = [0u8; 8192];
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read error: {}", e))?;
        if n == 0 {
            break;
        }
        total += n;
        if total > config.max_response_bytes {
            return Err(format!(
                "response too large (>{} bytes)",
                config.max_response_bytes
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Perform a controlled HTTP POST request with JSON body.
///
/// Same security enforcement as controlled_get.
pub fn controlled_post(
    url: &str,
    body: &str,
    content_type: &str,
    config: &ControlledFetchConfig,
) -> Result<String, String> {
    let host = extract_host(url).ok_or_else(|| format!("cannot parse host from URL: {}", url))?;

    if is_dangerous_host(&host) {
        return Err(format!(
            "BLOCKED: dangerous target '{}' — internal/metadata endpoints are never allowed",
            host
        ));
    }

    sandbox::check_domain_allowed(url, &config.domain_allowlist)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_millis(config.timeout_ms))
        .timeout_read(Duration::from_millis(config.timeout_ms))
        .user_agent(&config.user_agent)
        .redirects(5)
        .build();

    let response = agent
        .post(url)
        .set("Content-Type", content_type)
        .send_string(body)
        .map_err(|e| format!("controlled_fetch POST failed: {}", e))?;

    let mut buf = Vec::with_capacity(config.max_response_bytes.min(65536));
    let mut reader = response.into_reader();
    let mut total = 0usize;
    loop {
        let mut chunk = [0u8; 8192];
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("read error: {}", e))?;
        if n == 0 {
            break;
        }
        total += n;
        if total > config.max_response_bytes {
            return Err(format!(
                "response too large (>{} bytes)",
                config.max_response_bytes
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// MCP-facing controlled fetch — wraps result in security boundary.
pub fn mcp_controlled_fetch(
    url: &str,
    method: &str,
    body: Option<&str>,
    config: &ControlledFetchConfig,
) -> String {
    let result = match method.to_uppercase().as_str() {
        "GET" => controlled_get(url, config),
        "POST" => {
            let body_str = body.unwrap_or("{}");
            controlled_post(url, body_str, "application/json", config)
        }
        _ => Err(format!(
            "unsupported method: {} (only GET/POST allowed)",
            method
        )),
    };

    match result {
        Ok(data) => {
            let json_data = json!({
                "url": url,
                "method": method,
                "body": data,
                "bytes": data.len(),
            });
            security::wrap_secure(&json_data, "controlled_fetch")
        }
        Err(e) => security::wrap_secure(&json!({"error": e, "url": url}), "controlled_fetch:error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_localhost() {
        assert!(is_dangerous_host("localhost"));
        assert!(is_dangerous_host("127.0.0.1"));
        assert!(is_dangerous_host("0.0.0.0"));
        assert!(is_dangerous_host("[::1]"));
        assert!(is_dangerous_host("127.0.0.2"));
    }

    #[test]
    fn blocks_metadata_endpoints() {
        assert!(is_dangerous_host("169.254.169.254"));
        assert!(is_dangerous_host("metadata.google.internal"));
    }

    #[test]
    fn blocks_internal_ips() {
        assert!(is_dangerous_host("10.0.0.1"));
        assert!(is_dangerous_host("10.255.255.255"));
        assert!(is_dangerous_host("172.16.0.1"));
        assert!(is_dangerous_host("172.31.255.255"));
        assert!(is_dangerous_host("192.168.1.1"));
        assert!(is_dangerous_host("192.168.0.100"));
    }

    #[test]
    fn allows_public_hosts() {
        assert!(!is_dangerous_host("example.com"));
        assert!(!is_dangerous_host("api.github.com"));
        assert!(!is_dangerous_host("8.8.8.8"));
        assert!(!is_dangerous_host("1.1.1.1"));
    }

    #[test]
    fn blocks_octal_and_decimal_localhost() {
        assert!(is_dangerous_host("0177.0.0.1"));
        assert!(is_dangerous_host("2130706433"));
    }

    #[test]
    fn controlled_get_blocks_dangerous_url() {
        let config = ControlledFetchConfig::default();
        let result = controlled_get("http://169.254.169.254/latest/meta-data/", &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BLOCKED"));
    }

    #[test]
    fn controlled_get_blocks_localhost() {
        let config = ControlledFetchConfig::default();
        let result = controlled_get("http://localhost:8080/admin", &config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BLOCKED"));
    }

    #[test]
    fn controlled_post_blocks_dangerous() {
        let config = ControlledFetchConfig::default();
        let result = controlled_post(
            "http://10.0.0.1/internal-api",
            "{}",
            "application/json",
            &config,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("BLOCKED"));
    }

    #[test]
    fn extract_host_works() {
        assert_eq!(
            extract_host("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host("http://127.0.0.1:8080/"),
            Some("127.0.0.1".to_string())
        );
        assert_eq!(
            extract_host("https://user:pass@host.io/x"),
            Some("host.io".to_string())
        );
    }

    #[test]
    fn mcp_controlled_fetch_wraps_error() {
        let config = ControlledFetchConfig::default();
        let result = mcp_controlled_fetch("http://localhost/", "GET", None, &config);
        assert!(result.contains("BLOCKED"));
        assert!(result.contains("security-boundary"));
    }

    #[test]
    fn mcp_controlled_fetch_rejects_unsupported_method() {
        let config = ControlledFetchConfig::default();
        let result = mcp_controlled_fetch("https://example.com", "DELETE", None, &config);
        assert!(result.contains("unsupported method"));
    }

    #[test]
    fn blocks_ipv6_internal() {
        assert!(is_dangerous_host("fc00::1"));
        assert!(is_dangerous_host("fd00::1"));
        assert!(is_dangerous_host("fe80::1"));
    }
}
