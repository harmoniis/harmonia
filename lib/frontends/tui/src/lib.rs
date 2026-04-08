#[cfg(unix)]
pub mod terminal;
#[cfg(not(unix))]
pub mod terminal {
    pub fn init() -> Result<(), String> {
        Err("tui frontend requires Unix domain sockets and is unavailable on this platform".into())
    }

    pub fn poll() -> Vec<(String, String)> {
        Vec::new()
    }

    pub fn send(_sub_channel: &str, _payload: &str) {}

    pub fn shutdown() {}
}

// ── Client-side TUI modules ──────────────────────────────────────────

pub mod theme;
pub mod bridge;
pub mod render;
pub mod spinner;
pub mod input;
pub mod autocomplete;
pub mod prompt;
pub mod commands;
pub mod session;

pub use input::InputCallbacks;
pub use session::{run, SessionHost};
