use harmonia_actor_protocol::{ActorId, ActorKind, HarmoniaMessage};
use harmonia_observability::ObsMsg;
use ractor::{ActorRef, RpcReplyPort};

use crate::actors::{ComponentMsg, MatrixMsg};

// ── RuntimeSupervisor messages ───────────────────────────────────────
//
// Variants that expect a reply use tuple style so ractor::call_t! works.

pub enum RuntimeMsg {
    /// Register a new actor. call_t!(sup, Register, kind, timeout) → ActorId
    Register(ActorKind, RpcReplyPort<ActorId>),
    /// Deregister an actor. call_t!(sup, Deregister, id, timeout) → bool
    Deregister(ActorId, RpcReplyPort<bool>),
    /// Post a message into the system (fire-and-forget).
    Post {
        source: ActorId,
        target: ActorId,
        payload_sexp: String,
    },
    /// Heartbeat from an actor (fire-and-forget).
    Heartbeat { id: ActorId, bytes_delta: u64 },
    /// SBCL called (:drain). call_t!(sup, DrainSbcl, timeout) → String
    DrainSbcl(RpcReplyPort<String>),
    /// Get actor state as sexp. call_t!(sup, GetState, id, timeout) → String
    GetState(ActorId, RpcReplyPort<String>),
    /// List all actors as sexp. call_t!(sup, ListAll, timeout) → String
    ListAll(RpcReplyPort<String>),
    /// Component dispatch: route a sexp command to the named component.
    /// call_t!(sup, ComponentCall, (component, sexp), timeout) → String
    ComponentCall(String, String, RpcReplyPort<String>),
    /// Register a component actor for supervisor restart tracking (fire-and-forget).
    RegisterComponent(String, ActorRef<ComponentMsg>),
    /// Register the matrix actor (separate message type).
    RegisterMatrixActor(ActorRef<MatrixMsg>),
    /// Register the observability actor (separate message type — ObsMsg, not ComponentMsg).
    RegisterObsActor(ActorRef<ObsMsg>),
    /// Inject the DynamicRegistry so supervisor can re-register actors on restart.
    SetDynamicRegistry(crate::dynamic_registry::SharedDynamicRegistry),
    /// Inject the TopicBus so supervisor can unsubscribe crashed actors.
    SetTopicBus(crate::topic_bus::SharedTopicBus),
    /// List all modules and their status. call_t!(sup, ListModules, timeout) → String
    ListModules(RpcReplyPort<String>),
    /// Load a module by name. call_t!(sup, LoadModule, name, timeout) → String
    LoadModule(String, RpcReplyPort<String>),
    /// Unload a module by name. call_t!(sup, UnloadModule, name, timeout) → String
    UnloadModule(String, RpcReplyPort<String>),
    /// Initiate graceful shutdown (fire-and-forget).
    Shutdown,
}

// ── SbclBridgeActor messages ─────────────────────────────────────────

pub enum BridgeMsg {
    /// Enqueue a message for SBCL to drain (fire-and-forget).
    Enqueue { msg: HarmoniaMessage },
    /// Drain all queued messages as sexp. call_t!(bridge, Drain, timeout) → String
    Drain(RpcReplyPort<String>),
}
