//! Stealth engine: anti-detection for headless Chrome.
//!
//! Implements Scrapling-like techniques to make Chrome indistinguishable from
//! a human-operated browser. Generates JavaScript to inject via CDP that:
//! - Overrides navigator.webdriver
//! - Fakes navigator.plugins and mimeTypes
//! - Spoofs WebGL vendor/renderer
//! - Adds canvas fingerprint noise
//! - Overrides chrome.runtime
//! - Matches navigator.languages to locale
//!
//! Also provides human-like behavior simulation: typing, mouse movement,
//! scrolling with randomized timing.

use std::time::Duration;

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

/// Simple seed from system time (avoids rand dependency).
fn simple_seed() -> usize {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| (d.as_nanos() as usize) ^ (d.as_millis() as usize))
        .unwrap_or(42)
}

/// Generate a random-ish value in [min, max].
fn random_range(min: u64, max: u64) -> u64 {
    if max <= min {
        return min;
    }
    let seed = simple_seed() as u64;
    min + (seed % (max - min + 1))
}

/// Canvas noise injection script fragment.
const CANVAS_NOISE_SCRIPT: &str = r#"
    // 8. Canvas fingerprint noise
    const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
    HTMLCanvasElement.prototype.toDataURL = function(type, quality) {
        const ctx = this.getContext('2d');
        if (ctx && this.width > 0 && this.height > 0) {
            try {
                const imageData = ctx.getImageData(0, 0, this.width, this.height);
                const data = imageData.data;
                for (let i = 0; i < Math.min(10, data.length / 4); i++) {
                    const idx = (Math.floor(Math.random() * (data.length / 4))) * 4;
                    data[idx] = data[idx] ^ 1;
                }
                ctx.putImageData(imageData, 0, 0);
            } catch(e) {}
        }
        return origToDataURL.call(this, type, quality);
    };

    const origToBlob = HTMLCanvasElement.prototype.toBlob;
    HTMLCanvasElement.prototype.toBlob = function(callback, type, quality) {
        const ctx = this.getContext('2d');
        if (ctx && this.width > 0 && this.height > 0) {
            try {
                const imageData = ctx.getImageData(0, 0, this.width, this.height);
                const data = imageData.data;
                for (let i = 0; i < Math.min(10, data.length / 4); i++) {
                    const idx = (Math.floor(Math.random() * (data.length / 4))) * 4;
                    data[idx] = data[idx] ^ 1;
                }
                ctx.putImageData(imageData, 0, 0);
            } catch(e) {}
        }
        return origToBlob.call(this, callback, type, quality);
    };
"#;

/// Generate the stealth JavaScript to inject before page load.
///
/// This script patches navigator properties, WebGL, canvas, and other
/// detection vectors to make headless Chrome indistinguishable from
/// a regular browser.
pub fn stealth_script(config: &StealthConfig) -> String {
    let hw_concurrency = 4 + (simple_seed() % 13); // 4-16
    let device_memory = [4, 8, 8, 16][simple_seed() % 4];
    let locale_short = config.locale.split('-').next().unwrap_or("en");

    let canvas_block = if config.canvas_noise {
        CANVAS_NOISE_SCRIPT
    } else {
        "// Canvas noise disabled"
    };

    format!(
        r#"
// === Harmonia Stealth Engine ===

(() => {{
    'use strict';

    // 1. Override navigator.webdriver
    Object.defineProperty(navigator, 'webdriver', {{
        get: () => false,
        configurable: true,
    }});
    try {{ delete Object.getPrototypeOf(navigator).webdriver; }} catch(e) {{}}

    // 2. Override navigator.plugins
    Object.defineProperty(navigator, 'plugins', {{
        get: () => {{
            const plugins = [
                {{ name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' }},
                {{ name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: '' }},
                {{ name: 'Native Client', filename: 'internal-nacl-plugin', description: '' }},
            ];
            const arr = Object.create(PluginArray.prototype);
            for (let i = 0; i < plugins.length; i++) {{
                const p = Object.create(Plugin.prototype);
                Object.defineProperties(p, {{
                    name: {{ get: () => plugins[i].name }},
                    filename: {{ get: () => plugins[i].filename }},
                    description: {{ get: () => plugins[i].description }},
                    length: {{ get: () => 1 }},
                }});
                arr[i] = p;
            }}
            Object.defineProperty(arr, 'length', {{ get: () => plugins.length }});
            arr.item = (i) => arr[i];
            arr.namedItem = (name) => Array.from({{ length: plugins.length }}, (_, i) => arr[i]).find(p => p.name === name);
            arr.refresh = () => {{}};
            return arr;
        }},
        configurable: true,
    }});

    // 3. Override navigator.mimeTypes
    Object.defineProperty(navigator, 'mimeTypes', {{
        get: () => {{
            const types = [
                {{ type: 'application/pdf', suffixes: 'pdf', description: 'Portable Document Format' }},
                {{ type: 'application/x-google-chrome-pdf', suffixes: 'pdf', description: 'Portable Document Format' }},
            ];
            const arr = Object.create(MimeTypeArray.prototype);
            for (let i = 0; i < types.length; i++) {{
                const m = Object.create(MimeType.prototype);
                Object.defineProperties(m, {{
                    type: {{ get: () => types[i].type }},
                    suffixes: {{ get: () => types[i].suffixes }},
                    description: {{ get: () => types[i].description }},
                }});
                arr[i] = m;
            }}
            Object.defineProperty(arr, 'length', {{ get: () => types.length }});
            arr.item = (i) => arr[i];
            arr.namedItem = (name) => Array.from({{ length: types.length }}, (_, i) => arr[i]).find(m => m.type === name);
            return arr;
        }},
        configurable: true,
    }});

    // 4. Override navigator.languages
    Object.defineProperty(navigator, 'languages', {{
        get: () => ['{locale}', '{locale_short}'],
        configurable: true,
    }});

    // 5. Override chrome.runtime to prevent headless detection
    if (!window.chrome) window.chrome = {{}};
    window.chrome.runtime = {{
        connect: function() {{ return {{ onMessage: {{ addListener: function() {{}} }}, postMessage: function() {{}} }}; }},
        sendMessage: function(msg, cb) {{ if (cb) cb(); }},
        onMessage: {{ addListener: function() {{}} }},
        onConnect: {{ addListener: function() {{}} }},
        id: undefined,
    }};

    // 6. Override navigator.permissions.query
    if (navigator.permissions) {{
        const originalQuery = navigator.permissions.query.bind(navigator.permissions);
        navigator.permissions.query = (parameters) => {{
            if (parameters.name === 'notifications') {{
                return Promise.resolve({{ state: Notification.permission }});
            }}
            return originalQuery(parameters);
        }};
    }}

    // 7. WebGL vendor/renderer spoofing
    const patchWebGL = (proto) => {{
        const orig = proto.getParameter;
        proto.getParameter = function(param) {{
            if (param === 37445) return '{webgl_vendor}';
            if (param === 37446) return '{webgl_renderer}';
            return orig.call(this, param);
        }};
    }};
    if (typeof WebGLRenderingContext !== 'undefined') patchWebGL(WebGLRenderingContext.prototype);
    if (typeof WebGL2RenderingContext !== 'undefined') patchWebGL(WebGL2RenderingContext.prototype);

    {canvas_block}

    // 9. Override navigator.hardwareConcurrency
    Object.defineProperty(navigator, 'hardwareConcurrency', {{
        get: () => {hw_concurrency},
        configurable: true,
    }});

    // 10. Override navigator.deviceMemory
    Object.defineProperty(navigator, 'deviceMemory', {{
        get: () => {device_memory},
        configurable: true,
    }});

    // 11. Window dimensions
    Object.defineProperty(window, 'outerHeight', {{
        get: () => {viewport_h},
        configurable: true,
    }});
    Object.defineProperty(window, 'outerWidth', {{
        get: () => {viewport_w},
        configurable: true,
    }});

    // 12. Spoof connection type
    if (navigator.connection) {{
        try {{
            Object.defineProperty(navigator.connection, 'rtt', {{ get: () => 50 }});
            Object.defineProperty(navigator.connection, 'downlink', {{ get: () => 10 }});
            Object.defineProperty(navigator.connection, 'effectiveType', {{ get: () => '4g' }});
        }} catch(e) {{}}
    }}

}})();
"#,
        locale = config.locale,
        locale_short = locale_short,
        webgl_vendor = config.webgl_vendor.replace('\'', "\\'"),
        webgl_renderer = config.webgl_renderer.replace('\'', "\\'"),
        canvas_block = canvas_block,
        hw_concurrency = hw_concurrency,
        device_memory = device_memory,
        viewport_w = config.viewport.0,
        viewport_h = config.viewport.1,
    )
}

// ---- Human-like timing simulation ----

/// Simulate a human-like delay (50-300ms).
pub fn human_delay() {
    let ms = random_range(50, 300);
    std::thread::sleep(Duration::from_millis(ms));
}

/// Simulate a short action delay (20-100ms) for rapid interactions.
pub fn short_delay() {
    let ms = random_range(20, 100);
    std::thread::sleep(Duration::from_millis(ms));
}

/// Simulate a page load wait (500-2000ms).
pub fn page_load_delay() {
    let ms = random_range(500, 2000);
    std::thread::sleep(Duration::from_millis(ms));
}

// ---- CDP interaction functions (chrome feature only) ----

/// CDP-level stealth operations that require a live Chrome tab.
#[cfg(feature = "chrome")]
pub mod cdp {
    use super::*;
    use headless_chrome::Tab;
    use std::sync::Arc;

    /// Inject stealth scripts into a tab's current page.
    ///
    /// Call this after navigation completes to patch the page's JavaScript
    /// context with anti-detection overrides.
    pub fn inject_stealth(tab: &Arc<Tab>, config: &StealthConfig) -> Result<(), String> {
        let script = stealth_script(config);
        tab.evaluate(&script, false)
            .map_err(|e| format!("stealth injection failed: {}", e))?;
        Ok(())
    }

    /// Simulate human-like mouse movement to coordinates using eased steps.
    pub fn move_mouse(tab: &Arc<Tab>, target_x: f64, target_y: f64) -> Result<(), String> {
        let steps = 5 + (simple_seed() % 6);
        let start_x = 0.0;
        let start_y = 0.0;

        for i in 1..=steps {
            let t = i as f64 / steps as f64;
            // Ease-in-out cubic for natural movement
            let t_eased = if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0_f64).powi(3) / 2.0
            };
            let x = start_x + (target_x - start_x) * t_eased;
            let y = start_y + (target_y - start_y) * t_eased;

            let jitter_x = ((simple_seed() % 5) as f64 - 2.0) * 0.5;
            let jitter_y = ((simple_seed() % 5) as f64 - 2.0) * 0.5;

            tab.evaluate(
                &format!(
                    "document.dispatchEvent(new MouseEvent('mousemove', \
                     {{clientX: {}, clientY: {}, bubbles: true}}))",
                    x + jitter_x,
                    y + jitter_y
                ),
                false,
            )
            .map_err(|e| format!("mouse move failed: {}", e))?;

            std::thread::sleep(Duration::from_millis(random_range(10, 30)));
        }
        Ok(())
    }

    /// Simulate human-like typing character by character with variable delays.
    pub fn type_text(tab: &Arc<Tab>, selector: &str, text: &str) -> Result<(), String> {
        let escaped_sel = selector.replace('\'', "\\'");

        // Focus the element
        tab.evaluate(
            &format!("document.querySelector('{}')?.focus()", escaped_sel),
            false,
        )
        .map_err(|e| format!("focus failed: {}", e))?;

        short_delay();

        for ch in text.chars() {
            let escaped = match ch {
                '\'' => "\\'".to_string(),
                '\\' => "\\\\".to_string(),
                '\n' => "\\n".to_string(),
                _ => ch.to_string(),
            };

            tab.evaluate(
                &format!(
                    "(() => {{ \
                        const el = document.activeElement; \
                        if (!el) return; \
                        el.dispatchEvent(new KeyboardEvent('keydown', {{key: '{ch}', bubbles: true}})); \
                        el.dispatchEvent(new InputEvent('input', {{data: '{ch}', inputType: 'insertText', bubbles: true}})); \
                        el.dispatchEvent(new KeyboardEvent('keyup', {{key: '{ch}', bubbles: true}})); \
                    }})()",
                    ch = escaped
                ),
                false,
            )
            .map_err(|e| format!("typing failed: {}", e))?;

            // Variable delay between keystrokes (30-120ms)
            std::thread::sleep(Duration::from_millis(random_range(30, 120)));
        }
        Ok(())
    }

    /// Simulate smooth scrolling by a given delta.
    pub fn scroll(tab: &Arc<Tab>, delta_y: i32) -> Result<(), String> {
        let steps = 3 + (simple_seed() % 4);
        let step_delta = delta_y / steps as i32;

        for _ in 0..steps {
            tab.evaluate(
                &format!("window.scrollBy({{top: {}, behavior: 'auto'}})", step_delta),
                false,
            )
            .map_err(|e| format!("scroll failed: {}", e))?;

            std::thread::sleep(Duration::from_millis(random_range(30, 80)));
        }
        Ok(())
    }

    /// Click an element by CSS selector with human-like behavior.
    pub fn click_element(tab: &Arc<Tab>, selector: &str) -> Result<(), String> {
        let escaped = selector.replace('\'', "\\'");

        // Scroll element into view
        tab.evaluate(
            &format!(
                "(() => {{ \
                    const el = document.querySelector('{sel}'); \
                    if (!el) throw new Error('element not found: {sel}'); \
                    el.scrollIntoView({{behavior: 'smooth', block: 'center'}}); \
                    return true; \
                }})()",
                sel = escaped
            ),
            false,
        )
        .map_err(|e| format!("scroll into view failed: {}", e))?;

        human_delay();

        // Dispatch mouse events and click
        tab.evaluate(
            &format!(
                "(() => {{ \
                    const el = document.querySelector('{sel}'); \
                    if (!el) throw new Error('element not found'); \
                    el.dispatchEvent(new MouseEvent('mousedown', {{bubbles: true}})); \
                    el.dispatchEvent(new MouseEvent('mouseup', {{bubbles: true}})); \
                    el.click(); \
                    return true; \
                }})()",
                sel = escaped
            ),
            false,
        )
        .map_err(|e| format!("click failed: {}", e))?;

        Ok(())
    }

    /// Wait for a CSS selector to appear in the DOM.
    pub fn wait_for_selector(
        tab: &Arc<Tab>,
        selector: &str,
        timeout_ms: u64,
    ) -> Result<(), String> {
        let escaped = selector.replace('\'', "\\'").replace('\\', "\\\\");
        let poll_interval = 100u64;
        let max_polls = timeout_ms / poll_interval;

        for _ in 0..max_polls {
            if let Ok(val) =
                tab.evaluate(&format!("!!document.querySelector('{}')", escaped), false)
            {
                if val.value.as_ref().and_then(|v| v.as_bool()) == Some(true) {
                    return Ok(());
                }
            }
            std::thread::sleep(Duration::from_millis(poll_interval));
        }

        Err(format!("timeout waiting for selector: {}", selector))
    }

    /// Get text content of an element by CSS selector.
    pub fn get_text(tab: &Arc<Tab>, selector: &str) -> Result<String, String> {
        let escaped = selector.replace('\'', "\\'");
        let result = tab
            .evaluate(
                &format!(
                    "(() => {{ \
                        const el = document.querySelector('{sel}'); \
                        return el ? el.textContent : null; \
                    }})()",
                    sel = escaped
                ),
                false,
            )
            .map_err(|e| format!("get_text failed: {}", e))?;

        result
            .value
            .as_ref()
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("element not found or no text: {}", selector))
    }

    /// Take a screenshot of the visible viewport, returning base64 PNG.
    pub fn screenshot(tab: &Arc<Tab>) -> Result<String, String> {
        let png_data = tab
            .capture_screenshot(
                headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
                None,
                None,
                true,
            )
            .map_err(|e| format!("screenshot failed: {}", e))?;

        Ok(base64_encode(&png_data))
    }

    /// Evaluate arbitrary JavaScript and return the result as a string.
    pub fn evaluate_js(tab: &Arc<Tab>, expression: &str) -> Result<String, String> {
        let result = tab
            .evaluate(expression, false)
            .map_err(|e| format!("evaluate_js failed: {}", e))?;

        Ok(result
            .value
            .as_ref()
            .map(|v| {
                if let Some(s) = v.as_str() {
                    s.to_string()
                } else {
                    v.to_string()
                }
            })
            .unwrap_or_else(|| "undefined".to_string()))
    }

    /// Simple base64 encoding without external dependency.
    fn base64_encode(data: &[u8]) -> String {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;
            result.push(CHARS[((n >> 18) & 0x3F) as usize] as char);
            result.push(CHARS[((n >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                result.push(CHARS[((n >> 6) & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
            if chunk.len() > 2 {
                result.push(CHARS[(n & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
        }
        result
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
    fn stealth_script_contains_all_overrides() {
        let config = StealthConfig::default();
        let script = stealth_script(&config);
        assert!(script.contains("navigator.webdriver"));
        assert!(script.contains("navigator.plugins"));
        assert!(script.contains("navigator.mimeTypes"));
        assert!(script.contains("navigator.languages"));
        assert!(script.contains("chrome.runtime"));
        assert!(script.contains("navigator.permissions"));
        assert!(script.contains("WebGLRenderingContext"));
        assert!(script.contains("WebGL2RenderingContext"));
        assert!(script.contains("HTMLCanvasElement"));
        assert!(script.contains("hardwareConcurrency"));
        assert!(script.contains("deviceMemory"));
        assert!(script.contains("outerHeight"));
        assert!(script.contains("outerWidth"));
    }

    #[test]
    fn stealth_script_uses_config_values() {
        let config = StealthConfig {
            user_agent: "TestUA".to_string(),
            viewport: (1280, 720),
            locale: "fr-FR".to_string(),
            timezone: "Europe/Paris".to_string(),
            webgl_vendor: "TestVendor".to_string(),
            webgl_renderer: "TestRenderer".to_string(),
            canvas_noise: false,
            human_delays: true,
        };
        let script = stealth_script(&config);
        assert!(script.contains("fr-FR"));
        assert!(script.contains("'fr'"));
        assert!(script.contains("TestVendor"));
        assert!(script.contains("TestRenderer"));
        assert!(script.contains("Canvas noise disabled"));
        assert!(script.contains("1280"));
        assert!(script.contains("720"));
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

    #[test]
    fn random_range_in_bounds() {
        for _ in 0..50 {
            let v = random_range(50, 300);
            assert!(v >= 50 && v <= 300, "value {} out of range", v);
        }
    }

    #[test]
    fn random_range_equal_bounds() {
        assert_eq!(random_range(42, 42), 42);
    }
}
