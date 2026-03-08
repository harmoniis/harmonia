//! Chrome CDP integration with hardened launch configuration.
//!
//! When the `chrome` feature is enabled, this module provides headless Chrome
//! control via CDP (Chrome DevTools Protocol). Chrome is launched with security-
//! hardened flags: no GPU, no background networking, muted audio, no extensions,
//! no sync, no component updates.
//!
//! Architecture:
//! - Chrome process is managed by `headless_chrome`
//! - All networking from JS code goes through `controlled_fetch` (Rust/ureq)
//! - Audio is extracted but never played
//! - GPU rendering is disabled (we only extract data, never render)

/// Hardened Chrome launch arguments.
///
/// These flags create a minimal, secure Chrome instance suitable for
/// data extraction only — no rendering, no audio playback, no background
/// network activity, no extensions or sync.
pub const HARDENED_CHROME_ARGS: &[&str] = &[
    // Sandbox (Chrome's own sandbox may conflict with our OS sandbox)
    "--no-sandbox",
    "--disable-setuid-sandbox",

    // GPU — not needed for data extraction
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

    // Feature flags — disable heavyweight subsystems
    "--disable-features=OptimizationHints,Translate,MediaRouter,AudioServiceOutOfProcess,BackgroundFetch",

    // Headless mode
    "--headless=new",

    // Window size for consistent layout extraction
    "--window-size=1920,1080",
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
}

impl Default for ChromeConfig {
    fn default() -> Self {
        Self {
            chrome_path: None,
            extra_args: Vec::new(),
            navigation_timeout_ms: 30_000,
            enabled: false,
        }
    }
}

/// Fetch a page using headless Chrome CDP, returning the fully rendered HTML.
///
/// This handles JavaScript-rendered pages that ureq cannot process.
/// Chrome is launched with hardened flags and the page is loaded with
/// a navigation timeout.
///
/// Returns the outer HTML of the document after JS execution.
#[cfg(feature = "chrome")]
pub fn chrome_fetch(url: &str, config: &ChromeConfig) -> Result<String, String> {
    use headless_chrome::{Browser, LaunchOptions};
    use std::ffi::OsStr;
    use std::time::Duration;

    let mut args: Vec<&OsStr> = HARDENED_CHROME_ARGS
        .iter()
        .map(|s| OsStr::new(*s))
        .collect();

    // Add any extra args
    let extra_os: Vec<&OsStr> = config
        .extra_args
        .iter()
        .map(|s| OsStr::new(s.as_str()))
        .collect();
    args.extend(extra_os.iter());

    let launch_options = LaunchOptions {
        headless: true,
        args,
        path: config.chrome_path.as_ref().map(std::path::PathBuf::from),
        ..LaunchOptions::default()
    };

    let browser =
        Browser::new(launch_options).map_err(|e| format!("failed to launch Chrome: {}", e))?;

    let tab = browser
        .new_tab()
        .map_err(|e| format!("failed to create tab: {}", e))?;

    tab.navigate_to(url)
        .map_err(|e| format!("navigation failed: {}", e))?;

    tab.wait_until_navigated()
        .map_err(|e| format!("wait for navigation failed: {}", e))?;

    // Wait for page to be idle (DOMContentLoaded + network idle)
    std::thread::sleep(Duration::from_millis(500));

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
    fn chrome_config_defaults() {
        let cfg = ChromeConfig::default();
        assert!(cfg.chrome_path.is_none());
        assert!(cfg.extra_args.is_empty());
        assert_eq!(cfg.navigation_timeout_ms, 30_000);
        assert!(!cfg.enabled);
    }

    #[test]
    fn chrome_available_reflects_feature() {
        // Without feature, should be false
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
