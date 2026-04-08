/// Every frontend implements this. Defines the channel contract.
///
/// The gateway routes messages through this trait — each frontend (TUI,
/// Telegram, Slack, MQTT, etc.) provides its own implementation.
/// No global state: each instance is a value-owned channel handle.
pub trait FrontendChannel: Send + Sync {
    /// Channel kind (tui, telegram, slack, mqtt, etc.)
    fn kind(&self) -> &str;

    /// Send a message to this frontend.
    fn send(&self, message: &str) -> Result<(), String>;

    /// Poll for new messages (non-blocking). Returns (address, payload) pairs.
    fn poll(&self) -> Vec<(String, String)>;
}
