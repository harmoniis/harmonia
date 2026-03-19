//! Distributed tracing for the Harmonia agent via LangSmith.
//!
//! Provides non-blocking, background trace submission to LangSmith's REST API.
//! Observability never blocks the agent — if LangSmith is unreachable, traces
//! are silently dropped. All FFI calls are safe to invoke even when tracing
//! is disabled or not yet initialized.

pub mod client;
pub(crate) mod config;
pub mod context;
pub mod ffi;
pub mod model;
pub(crate) mod sender;

// Re-export the public API for convenience
pub use ffi::{
    harmonia_observability_flush, harmonia_observability_init, harmonia_observability_shutdown,
    harmonia_observability_trace_end, harmonia_observability_trace_event,
    harmonia_observability_trace_start,
};
