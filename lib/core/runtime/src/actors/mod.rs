//! Actor wrappers for Harmonia components.
//!
//! Each wrapper is a ractor actor that owns a component's lifecycle.
//! pre_start initializes the component, handle() dispatches messages
//! to the component's public API, and supervision handles recovery.

mod gateway;
mod generated;
mod harmonic_matrix;
mod memory_field;
mod observability;
mod router;
mod signalograd;
mod tailnet;
mod vault;

pub use gateway::GatewayActor;
pub use generated::*;
pub use harmonic_matrix::{HarmonicMatrixActor, MatrixMsg};
pub use memory_field::MemoryFieldActor;
pub use observability::ObservabilityActor;
pub use router::RouterActor;
pub use signalograd::SignalogradActor;
pub use tailnet::TailnetActor;
pub use vault::VaultActor;

// ── Shared message type for all component actors ─────────────────────

use ractor::RpcReplyPort;

pub enum ComponentMsg {
    Tick,
    Dispatch(String, RpcReplyPort<String>),
    Shutdown,
}
