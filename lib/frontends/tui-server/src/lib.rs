//! Server-side TUI transport for the Harmonia agent.
//!
//! Runs *inside the agent process* and accepts connections from the operator's
//! interactive TUI session. The previous implementation used a module-level
//! `OnceLock<RwLock<TuiState>>` singleton; this one packages the server
//! state inside a struct that implements [`harmonia_frontend_trait::Frontend`],
//! so the runtime treats the TUI like any other actor in the frontend
//! registry.
//!
//! Phase 3a focuses on the local Unix-domain-socket transport (existing
//! behaviour). Phase 8 will plumb in a tailscale-userspace (`tsnet`)
//! transport so the operator can reach the same TUI over the agent owner's
//! tailnet from a different node.

pub mod uds;

pub use uds::TuiServer;
