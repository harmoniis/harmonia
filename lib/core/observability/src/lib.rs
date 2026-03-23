//! Distributed tracing for the Harmonia agent.
//!
//! Provider-agnostic: the observability core publishes to whatever provider
//! is configured (LangSmith, OpenObserve/OTLP, etc.). If no provider is
//! configured, traces are silently dropped.
//!
//! Architecture: ObservabilityActor is the single trace sink. All trace data
//! flows as ractor cast (fire-and-forget). The actor owns the sender thread,
//! sampling, and correlation state.

pub mod backend;
pub mod config;
pub mod ffi;
pub mod model;
pub mod providers;
pub mod sender;
pub mod traceable;

pub use backend::{FlushResult, TraceBackend};
pub use config::ObservabilityConfig;
pub use ffi::{
    get_config, get_obs_actor, harmonia_observability_enabled, harmonia_observability_flush,
    harmonia_observability_init, harmonia_observability_is_standard,
    harmonia_observability_is_verbose, harmonia_observability_shutdown, set_obs_actor,
    start_sender,
};
pub use model::{DottedOrderEntry, ObsMsg, TraceMessage};
pub use traceable::Traceable;
