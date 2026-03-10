pub mod frontend_ffi;
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
