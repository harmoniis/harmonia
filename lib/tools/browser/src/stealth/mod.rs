//! Stealth engine: anti-detection for headless Chrome.
//!
//! Implements Scrapling-like techniques to make Chrome indistinguishable from
//! a human-operated browser.

mod config;
mod script;
pub mod timing;

#[cfg(feature = "chrome")]
pub mod cdp;

// Re-export: config
pub use config::StealthConfig;

// Re-export: script generation
pub use script::stealth_script;

// Re-export: timing
pub use timing::{human_delay, page_load_delay, short_delay};
