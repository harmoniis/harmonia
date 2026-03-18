//! Harmonia Secure HTTP Fetch (`hfetch`)
//!
//! A secure HTTP client built on ureq with signal-integrity protection.
//! Provides SSRF prevention, injection detection, dissonance scoring,
//! and security boundary wrapping on all responses.
//!
//! Usable as both a Rust library and a standalone CLI tool.

use std::io::Read;
use std::net::IpAddr;

// ── SSRF Protection ────────────────────────────────────────────────────────

fn is_blocked_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()                              // 127.0.0.0/8
                || v4.is_private()                        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
                || v4.is_link_local()                     // 169.254.0.0/16
                || v4.is_unspecified()                    // 0.0.0.0
                || v4.octets()[0] == 169 && v4.octets()[1] == 254 // AWS metadata
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()                              // ::1
                || v6.is_unspecified()                    // ::
                || {
                    let segments = v6.segments();
                    segments[0] == 0xfc00                 // fc00::/7 unique local
                        || segments[0] == 0xfd00
                        || segments[0] == 0xfe80          // fe80::/10 link-local
                }
        }
    }
}

fn is_blocked_host(host: &str) -> bool {
    let lower = host.to_lowercase();
    let blocked_hosts = [
        "localhost",
        "metadata.google.internal",
        "metadata.google",
        "169.254.169.254",
        "127.0.0.1",
        "[::1]",
        "0.0.0.0",
        "0177.0.0.1",
        "0x7f.0.0.1",
        "2130706433",
    ];
    if blocked_hosts
        .iter()
        .any(|b| lower == *b || lower.starts_with(&format!("{}:", b)))
    {
        return true;
    }
    // Try parsing as IP directly, or strip brackets for IPv6
    let ip_candidate = lower
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(&lower);
    if let Ok(ip) = ip_candidate.parse::<IpAddr>() {
        return is_blocked_ip(&ip);
    }
    false
}

fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    // Handle bracketed IPv6: [::1]:8080 → [::1]
    let host = if host_port.starts_with('[') {
        match host_port.find(']') {
            Some(end) => &host_port[..=end],
            None => host_port,
        }
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn check_ssrf(url: &str) -> Result<(), String> {
    let host = extract_host(url).ok_or_else(|| "cannot extract host from URL".to_string())?;
    if is_blocked_host(&host) {
        return Err(format!(
            "SSRF protection: blocked request to internal/metadata host: {host}"
        ));
    }
    Ok(())
}

// ── Public API ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "PATCH" => Method::Patch,
            "DELETE" => Method::Delete,
            "HEAD" => Method::Head,
            _ => Method::Get,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchOptions {
    pub method: Method,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout_ms: u64,
    pub max_response_bytes: usize,
    pub auth_bearer: Option<String>,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            method: Method::Get,
            headers: Vec::new(),
            body: None,
            timeout_ms: 10_000,
            max_response_bytes: 2 * 1024 * 1024,
            auth_bearer: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FetchResponse {
    pub status: u16,
    pub body: String,
    pub dissonance: f64,
    pub injection_detected: bool,
    pub headers: Vec<(String, String)>,
}

pub fn fetch(url: &str, opts: &FetchOptions) -> Result<FetchResponse, String> {
    check_ssrf(url)?;

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(opts.timeout_ms))
        .build();

    let mut req = match opts.method {
        Method::Get => agent.get(url),
        Method::Post => agent.post(url),
        Method::Put => agent.put(url),
        Method::Patch => agent.request("PATCH", url),
        Method::Delete => agent.delete(url),
        Method::Head => agent.head(url),
    };

    req = req.set("User-Agent", "hfetch/0.1.8 (harmonia)");

    if let Some(ref token) = opts.auth_bearer {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    for (key, value) in &opts.headers {
        req = req.set(key, value);
    }

    let resp = if let Some(ref body) = opts.body {
        let content_type = opts
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .map(|(_, v)| v.as_str())
            .unwrap_or("application/json");
        req.set("Content-Type", content_type)
            .send_string(body)
            .map_err(|e| format!("request failed: {e}"))?
    } else {
        req.call().map_err(|e| format!("request failed: {e}"))?
    };

    let status = resp.status();

    let resp_headers: Vec<(String, String)> = resp
        .headers_names()
        .into_iter()
        .filter_map(|name| {
            resp.header(&name)
                .map(|val| (name.to_string(), val.to_string()))
        })
        .collect();

    let mut body_str = String::new();
    resp.into_reader()
        .take(opts.max_response_bytes as u64)
        .read_to_string(&mut body_str)
        .map_err(|e| format!("failed to read response body: {e}"))?;

    let report = harmonia_signal_integrity::scan_for_injection(&body_str);
    let dissonance = harmonia_signal_integrity::compute_dissonance(&report);

    Ok(FetchResponse {
        status,
        body: body_str,
        dissonance,
        injection_detected: report.injection_detected,
        headers: resp_headers,
    })
}

pub fn fetch_with_security_wrap(url: &str, opts: &FetchOptions) -> Result<FetchResponse, String> {
    let mut resp = fetch(url, opts)?;
    resp.body = harmonia_signal_integrity::wrap_secure(&resp.body, "hfetch");
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssrf_blocks_localhost() {
        assert!(check_ssrf("http://localhost/secret").is_err());
        assert!(check_ssrf("http://127.0.0.1/secret").is_err());
        assert!(check_ssrf("http://169.254.169.254/metadata").is_err());
        assert!(check_ssrf("http://[::1]/secret").is_err());
        assert!(check_ssrf("http://0.0.0.0/secret").is_err());
    }

    #[test]
    fn ssrf_allows_public() {
        assert!(check_ssrf("https://example.com").is_ok());
        assert!(check_ssrf("https://api.github.com/repos").is_ok());
    }

    #[test]
    fn method_roundtrip() {
        assert_eq!(Method::from_str("POST").as_str(), "POST");
        assert_eq!(Method::from_str("get").as_str(), "GET");
    }
}
