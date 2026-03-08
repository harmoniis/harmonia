//! Process isolation and resource limits for browser operations.
//!
//! Provides timeout-enforced sandboxed execution. Full OS-level sandboxing
//! (namespaces, cgroups, seccomp) can be layered on top in production.

use std::sync::mpsc;
use std::time::Duration;

/// Configuration for the sandbox execution environment.
pub struct SandboxConfig {
    /// Maximum memory the operation may use (advisory, not enforced at OS level yet).
    pub memory_limit_bytes: usize,
    /// Maximum wall-clock time in milliseconds before the operation is killed.
    pub timeout_ms: u64,
    /// If set, only these domains may be contacted.
    pub network_allowlist: Option<Vec<String>>,
    /// Whether filesystem access is permitted (default: false).
    pub allow_filesystem: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            memory_limit_bytes: 256 * 1024 * 1024, // 256 MB
            timeout_ms: 30_000,
            network_allowlist: None,
            allow_filesystem: false,
        }
    }
}

/// Execute a closure within sandbox constraints.
///
/// Currently enforces: wall-clock timeout via thread + channel.
/// The closure runs on a dedicated thread; if it does not complete
/// within `config.timeout_ms`, the caller receives an error.
pub fn sandboxed_exec<F, R>(config: &SandboxConfig, f: F) -> Result<R, String>
where
    F: FnOnce() -> Result<R, String> + Send + 'static,
    R: Send + 'static,
{
    let timeout = Duration::from_millis(config.timeout_ms);
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let _ = tx.send(f());
    });

    rx.recv_timeout(timeout)
        .map_err(|_| format!("browser sandbox timeout ({}ms)", config.timeout_ms))?
}

/// Check whether a URL's host is allowed by the network allowlist.
pub fn check_domain_allowed(url: &str, allowlist: &Option<Vec<String>>) -> Result<(), String> {
    let allowlist = match allowlist {
        Some(list) => list,
        None => return Ok(()), // No allowlist = allow all
    };

    // Extract host from URL
    let host = extract_host(url).ok_or_else(|| format!("cannot parse host from URL: {}", url))?;

    for allowed in allowlist {
        if host == *allowed || host.ends_with(&format!(".{}", allowed)) {
            return Ok(());
        }
    }

    Err(format!("domain '{}' not in network allowlist", host))
}

/// Extract the host portion from a URL string without pulling in the `url` crate.
fn extract_host(url: &str) -> Option<String> {
    // Skip scheme
    let after_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };

    // Strip userinfo if present
    let after_user = if let Some(pos) = after_scheme.find('@') {
        &after_scheme[pos + 1..]
    } else {
        after_scheme
    };

    // Take up to the first / or : or ? or #
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandboxed_exec_returns_result() {
        let cfg = SandboxConfig {
            timeout_ms: 5000,
            ..Default::default()
        };
        let result = sandboxed_exec(&cfg, || Ok(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn sandboxed_exec_timeout() {
        let cfg = SandboxConfig {
            timeout_ms: 50,
            ..Default::default()
        };
        let result = sandboxed_exec(&cfg, || {
            std::thread::sleep(Duration::from_millis(5000));
            Ok(42)
        });
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timeout"));
    }

    #[test]
    fn domain_allowlist_permits_allowed() {
        let list = Some(vec!["example.com".to_string()]);
        assert!(check_domain_allowed("https://example.com/page", &list).is_ok());
        assert!(check_domain_allowed("https://sub.example.com/page", &list).is_ok());
    }

    #[test]
    fn domain_allowlist_blocks_disallowed() {
        let list = Some(vec!["example.com".to_string()]);
        assert!(check_domain_allowed("https://evil.com/page", &list).is_err());
    }

    #[test]
    fn domain_allowlist_none_allows_all() {
        assert!(check_domain_allowed("https://anything.com/page", &None).is_ok());
    }

    #[test]
    fn extract_host_works() {
        assert_eq!(
            extract_host("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_host("http://FOO.BAR:8080/x"),
            Some("foo.bar".to_string())
        );
        assert_eq!(
            extract_host("https://user:pass@host.io/x"),
            Some("host.io".to_string())
        );
    }
}
