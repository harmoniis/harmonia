//! Distributed tracing for the Harmonia agent via LangSmith.
//!
//! Provides non-blocking, background trace submission to LangSmith's REST API.
//! Observability never blocks the agent — if LangSmith is unreachable, traces
//! are silently dropped. All FFI calls are safe to invoke even when tracing
//! is disabled or not yet initialized.

pub mod client;
pub mod config;
pub mod context;
pub mod ffi;
pub mod model;
pub mod sender;
