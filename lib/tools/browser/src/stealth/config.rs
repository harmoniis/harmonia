//! Stealth configuration and browser profile constants.

use super::timing::simple_seed;

/// Stealth configuration for anti-detection.
pub struct StealthConfig {
    /// User-Agent string (realistic Chrome version).
    pub user_agent: String,
    /// Viewport dimensions (width, height).
    pub viewport: (u32, u32),
    /// Browser locale.
    pub locale: String,
    /// Timezone ID (e.g., "America/New_York").
    pub timezone: String,
    /// WebGL vendor string.
    pub webgl_vendor: String,
    /// WebGL renderer string.
    pub webgl_renderer: String,
    /// Whether to enable canvas noise.
    pub canvas_noise: bool,
    /// Whether to enable human-like delays.
    pub human_delays: bool,
}

/// Pool of realistic Chrome user-agent strings.
const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0.0.0 Safari/537.36",
];

/// Realistic WebGL vendor/renderer pairs.
const WEBGL_PROFILES: &[(&str, &str)] = &[
    (
        "Google Inc. (Apple)",
        "ANGLE (Apple, ANGLE Metal Renderer: Apple M1, Unspecified Version)",
    ),
    (
        "Google Inc. (Apple)",
        "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)",
    ),
    (
        "Google Inc. (Apple)",
        "ANGLE (Apple, ANGLE Metal Renderer: Apple M3, Unspecified Version)",
    ),
    (
        "Google Inc. (NVIDIA)",
        "ANGLE (NVIDIA, NVIDIA GeForce RTX 3070 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    ),
    (
        "Google Inc. (NVIDIA)",
        "ANGLE (NVIDIA, NVIDIA GeForce RTX 4060 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    ),
    (
        "Google Inc. (Intel)",
        "ANGLE (Intel, Intel(R) UHD Graphics 630 Direct3D11 vs_5_0 ps_5_0, D3D11)",
    ),
];

/// Common viewport sizes for randomization.
const VIEWPORTS: &[(u32, u32)] = &[
    (1920, 1080),
    (1536, 864),
    (1440, 900),
    (1366, 768),
    (2560, 1440),
    (1680, 1050),
];

impl Default for StealthConfig {
    fn default() -> Self {
        let seed = simple_seed();
        let ua_idx = seed % USER_AGENTS.len();
        let gl_idx = seed % WEBGL_PROFILES.len();
        let vp_idx = seed % VIEWPORTS.len();

        Self {
            user_agent: USER_AGENTS[ua_idx].to_string(),
            viewport: VIEWPORTS[vp_idx],
            locale: "en-US".to_string(),
            timezone: "America/New_York".to_string(),
            webgl_vendor: WEBGL_PROFILES[gl_idx].0.to_string(),
            webgl_renderer: WEBGL_PROFILES[gl_idx].1.to_string(),
            canvas_noise: true,
            human_delays: true,
        }
    }
}

impl StealthConfig {
    /// Get the Chrome launch args needed for stealth mode.
    pub fn chrome_args(&self) -> Vec<String> {
        vec![
            format!("--user-agent={}", self.user_agent),
            format!("--window-size={},{}", self.viewport.0, self.viewport.1),
            format!("--lang={}", self.locale),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_values() {
        let config = StealthConfig::default();
        assert!(!config.user_agent.is_empty());
        assert!(config.viewport.0 > 0);
        assert!(config.viewport.1 > 0);
        assert!(!config.webgl_vendor.is_empty());
        assert!(!config.webgl_renderer.is_empty());
        assert!(config.canvas_noise);
        assert!(config.human_delays);
    }

    #[test]
    fn chrome_args_from_config() {
        let config = StealthConfig::default();
        let args = config.chrome_args();
        assert!(args.len() >= 3);
        assert!(args[0].starts_with("--user-agent="));
        assert!(args[1].starts_with("--window-size="));
        assert!(args[2].starts_with("--lang="));
    }
}
