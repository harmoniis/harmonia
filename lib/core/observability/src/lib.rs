//! Distributed tracing for the Harmonia agent via LangSmith.
//!
//! Provides non-blocking, background trace submission to LangSmith's REST API.
//! Observability never blocks the agent — if LangSmith is unreachable, traces
//! are silently dropped with exponential backoff on rate limits.

pub mod client;
pub(crate) mod config;
pub mod context;
pub mod ffi;
pub mod model;
pub(crate) mod sender;

pub use ffi::{
    harmonia_observability_enabled, harmonia_observability_flush, harmonia_observability_init,
    harmonia_observability_is_standard, harmonia_observability_is_verbose,
    harmonia_observability_shutdown, harmonia_observability_trace_end,
    harmonia_observability_trace_event, harmonia_observability_trace_start,
};
