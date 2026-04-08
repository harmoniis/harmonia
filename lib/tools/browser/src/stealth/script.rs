//! Stealth JavaScript generation for anti-detection injection.

use super::config::StealthConfig;
use super::timing::simple_seed;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
