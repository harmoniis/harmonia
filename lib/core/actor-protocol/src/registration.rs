//! Actor registration record — bookkeeping for the supervisor.

use crate::actor::{ActorId, ActorKind, ActorState};

pub struct ActorRegistration {
    pub id: ActorId,
    pub kind: ActorKind,
    pub state: ActorState,
    pub registered_at: u64,
    pub last_heartbeat: u64,
    pub stall_ticks: u32,
    pub message_count: u64,
}

// ActorRegistry has been removed -- it now lives in harmonia-runtime
// as the RuntimeSupervisor actor (lib/core/runtime/src/supervisor.rs).
