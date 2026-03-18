//! Chrome CDP integration with hardened launch configuration and stealth.
//!
//! When the `chrome` feature is enabled, this module provides headless Chrome
//! control via CDP (Chrome DevTools Protocol). Chrome is launched with security-
//! hardened flags and anti-detection stealth patches (Scrapling-like).
//!
//! Architecture:
//! - Chrome process is managed by `headless_chrome`
//! - Stealth scripts are injected post-navigation to defeat bot detection
//! - Human-like delays between interactions
//! - Session pooling for reuse across requests

use crate::stealth::StealthConfig;

/// Hardened Chrome launch arguments.
///
/// Security-hardened flags that create a minimal Chrome instance.
/// Combined with stealth flags (user-agent, viewport, lang) from StealthConfig.
pub const HARDENED_CHROME_ARGS: &[&str] = &[
    // Sandbox (Chrome's own sandbox may conflict with our OS sandbox)
    "--no-sandbox",
    "--disable-setuid-sandbox",

    // GPU — disabled for extraction, but keep software rendering hints
    // so WebGL queries return realistic values
    "--disable-gpu",
    "--disable-software-rasterizer",

    // Audio — extract sources only, never play
    "--mute-audio",
    "--disable-audio-output",

    // Background networking — ALL networking goes through controlled_fetch
    "--disable-background-networking",
    "--disable-background-timer-throttling",

    // Component updates — no auto-updating in sandbox
    "--disable-component-update",

    // Shared memory — use /tmp instead of /dev/shm (container-safe)
    "--disable-dev-shm-usage",

    // Extensions and sync — no plugins, no account sync
    "--disable-extensions",
    "--disable-sync",
    "--no-first-run",

    // Feature flags — disable heavyweight subsystems but NOT automation ones
    // (leaving AutomationControlled out so we can override it with stealth)
    "--disable-features=OptimizationHints,Translate,MediaRouter,AudioServiceOutOfProcess,BackgroundFetch",

    // Headless mode — use Chrome's "new" headless which is harder to detect
    "--headless=new",

    // Disable blink features that leak automation state
    "--disable-blink-features=AutomationControlled",
];

/// Chrome configuration for the browser tool.
pub struct ChromeConfig {
    /// Path to Chrome/Chromium binary. If None, auto-detect.
    pub chrome_path: Option<String>,
    /// Additional launch arguments beyond the hardened defaults.
    pub extra_args: Vec<String>,
    /// Timeout for page navigation in milliseconds.
    pub navigation_timeout_ms: u64,
    /// Whether to enable Chrome (requires `chrome` feature flag).
    pub enabled: bool,
    /// Stealth configuration for anti-detection.
    pub stealth: StealthConfig,
}

impl Default for ChromeConfig {
    fn default() -> Self {
        Self {
            chrome_path: None,
            extra_args: Vec::new(),
            navigation_timeout_ms: 30_000,
            enabled: false,
            stealth: StealthConfig::default(),
        }
    }
}

impl ChromeConfig {
    /// Build the full argument list: hardened args + stealth args + extra args.
    pub fn all_args(&self) -> Vec<String> {
        let mut args: Vec<String> = HARDENED_CHROME_ARGS.iter().map(|s| s.to_string()).collect();

        // Add stealth-specific args (user-agent, viewport, lang)
        args.extend(self.stealth.chrome_args());

        // Add any extra args
        args.extend(self.extra_args.clone());

        args
    }
}

/// Fetch a page using headless Chrome CDP with stealth anti-detection.
///
/// 1. Launches Chrome with hardened + stealth flags
/// 2. Navigates to the URL
/// 3. Injects stealth scripts to patch detection vectors
/// 4. Returns the fully rendered HTML
#[cfg(feature = "chrome")]
pub fn chrome_fetch(url: &str, config: &ChromeConfig) -> Result<String, String> {
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsString;

    let all_args = config.all_args();
    let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
    let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

    let launch_options = LaunchOptions {
        headless: true,
        args: args_ref,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_options).map_err(|e| format!("failed to launch Chrome: {}", e))?;

    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;

    // Navigate to URL
    tab.navigate_to(url)
        .map_err(|e| format!("navigation failed: {}", e))?;

    tab.wait_until_navigated()
        .map_err(|e| format!("wait for navigation failed: {}", e))?;

    // Wait for page to be idle, then inject stealth patches
    crate::stealth::page_load_delay();

    // Inject stealth scripts post-navigation to patch detection vectors
    crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;

    // Get the rendered HTML
    let html = tab
        .get_content()
        .map_err(|e| format!("failed to get page content: {}", e))?;

    Ok(html)
}

/// Stub when chrome feature is not enabled.
#[cfg(not(feature = "chrome"))]
pub fn chrome_fetch(_url: &str, _config: &ChromeConfig) -> Result<String, String> {
    Err("Chrome CDP not available: compile with --features chrome".to_string())
}

/// Fetch a page using a session-pooled Chrome instance with stealth.
///
/// Reuses an existing browser session if available, otherwise creates one.
/// This is more efficient for multiple requests to the same domain.
#[cfg(feature = "chrome")]
pub fn chrome_fetch_session(
    url: &str,
    session_key: &str,
    config: &ChromeConfig,
    pool: &crate::session::SessionPool,
) -> Result<String, String> {
    use headless_chrome::LaunchOptions;
    use std::ffi::OsString;

    // Ensure session exists
    if !pool.has_session(session_key) {
        let all_args = config.all_args();
        let os_args: Vec<OsString> = all_args.iter().map(|s| OsString::from(s)).collect();
        let args_ref: Vec<&std::ffi::OsStr> = os_args.iter().map(|s| s.as_os_str()).collect();

        let launch_opts = LaunchOptions {
            headless: true,
            args: args_ref,
            path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
            ..LaunchOptions::default()
        };

        pool.register(session_key, launch_opts)?;
    }

    // Use the session
    pool.with_session(session_key, |browser| {
        let tab = browser
            .new_tab()
            .map_err(|e| format!("failed to create tab: {}", e))?;

        tab.navigate_to(url)
            .map_err(|e| format!("navigation failed: {}", e))?;

        tab.wait_until_navigated()
            .map_err(|e| format!("wait for navigation failed: {}", e))?;

        crate::stealth::page_load_delay();
        crate::stealth::cdp::inject_stealth(&tab, &config.stealth)?;

        tab.get_content()
            .map_err(|e| format!("failed to get page content: {}", e))
    })
}

#[cfg(not(feature = "chrome"))]
pub fn chrome_fetch_session(
    _url: &str,
    _session_key: &str,
    _config: &ChromeConfig,
    _pool: &crate::session::SessionPool,
) -> Result<String, String> {
    Err("Chrome CDP not available: compile with --features chrome".to_string())
}

/// Check if Chrome CDP is available (compiled with feature flag).
pub fn chrome_available() -> bool {
    cfg!(feature = "chrome")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hardened_args_include_critical_flags() {
        let args_str: Vec<&str> = HARDENED_CHROME_ARGS.to_vec();
        assert!(args_str.contains(&"--disable-gpu"));
        assert!(args_str.contains(&"--disable-background-networking"));
        assert!(args_str.contains(&"--mute-audio"));
        assert!(args_str.contains(&"--disable-audio-output"));
        assert!(args_str.contains(&"--disable-extensions"));
        assert!(args_str.contains(&"--disable-sync"));
        assert!(args_str.contains(&"--no-sandbox"));
        assert!(args_str.contains(&"--disable-setuid-sandbox"));
        assert!(args_str.contains(&"--disable-component-update"));
        assert!(args_str.contains(&"--disable-dev-shm-usage"));
    }

    #[test]
    fn hardened_args_include_stealth_flags() {
        let args_str: Vec<&str> = HARDENED_CHROME_ARGS.to_vec();
        assert!(args_str.contains(&"--headless=new"));
        assert!(args_str.contains(&"--disable-blink-features=AutomationControlled"));
    }

    #[test]
    fn chrome_config_defaults() {
        let cfg = ChromeConfig::default();
        assert!(cfg.chrome_path.is_none());
        assert!(cfg.extra_args.is_empty());
        assert_eq!(cfg.navigation_timeout_ms, 30_000);
        assert!(!cfg.enabled);
        assert!(!cfg.stealth.user_agent.is_empty());
    }

    #[test]
    fn all_args_includes_hardened_and_stealth() {
        let cfg = ChromeConfig::default();
        let args = cfg.all_args();
        // Should have hardened args
        assert!(args.iter().any(|a| a == "--disable-gpu"));
        assert!(args.iter().any(|a| a == "--headless=new"));
        // Should have stealth args (user-agent, window-size, lang)
        assert!(args.iter().any(|a| a.starts_with("--user-agent=")));
        assert!(args.iter().any(|a| a.starts_with("--lang=")));
    }

    #[test]
    fn chrome_available_reflects_feature() {
        #[cfg(not(feature = "chrome"))]
        assert!(!chrome_available());
        #[cfg(feature = "chrome")]
        assert!(chrome_available());
    }

    #[test]
    #[cfg(not(feature = "chrome"))]
    fn chrome_fetch_without_feature_returns_error() {
        let cfg = ChromeConfig::default();
        let result = chrome_fetch("https://example.com", &cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not available"));
    }
}
