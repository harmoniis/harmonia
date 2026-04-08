//! CDP-level stealth operations that require a live Chrome tab.

#![cfg(feature = "chrome")]

use super::config::StealthConfig;
use super::script::stealth_script;
use super::timing::{human_delay, random_range, short_delay, simple_seed};
use headless_chrome::Tab;
use std::sync::Arc;
use std::time::Duration;

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
