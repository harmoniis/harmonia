//! Trace backend trait.
//!
//! The observability core publishes to whatever provider is active.
//! If none is configured, nothing is published. The core never imports
//! or knows about any specific provider.

use serde_json::Value;

/// Result of a batch flush.
pub enum FlushResult {
    Ok,
    RateLimited(String),
    Error(String),
}

/// A trace backend provider. Each provider (LangSmith, OTLP, etc.) implements
/// this trait. The sender thread holds one `Box<dyn TraceBackend>` and calls
/// `submit_batch` on flush.
pub trait TraceBackend: Send + 'static {
    fn submit_batch(&self, creates: &[Value], updates: &[Value]) -> FlushResult;
    fn name(&self) -> &'static str;
}
