//! Observability providers.
//!
//! Each provider is a self-contained module that implements `TraceBackend`.
//! `from_config` is the single dispatch point — config drives which provider
//! is instantiated. If no provider matches or validation fails, returns None
//! and observability silently does not publish.

pub mod langsmith;
pub mod otlp;

use crate::backend::TraceBackend;
use crate::config::ObservabilityConfig;

/// Instantiate the configured provider. Returns None if the backend is
/// unconfigured, unknown, or fails validation (e.g., missing credentials).
pub fn from_config(config: &ObservabilityConfig) -> Option<Box<dyn TraceBackend>> {
    match config.backend.as_str() {
        "langsmith" => langsmith::LangSmith::from_config(config).map(|p| Box::new(p) as _),
        "otlp" | "openobserve" => otlp::Otlp::from_config(config).map(|p| Box::new(p) as _),
        "" | "none" | "disabled" => None,
        other => {
            eprintln!("[WARN] [observability] Unknown provider: {other}");
            None
        }
    }
}
