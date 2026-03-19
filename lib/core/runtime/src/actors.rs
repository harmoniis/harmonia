//! Actor wrappers for Harmonia components.
//!
//! Each wrapper is a ractor actor that owns a component's lifecycle.
//! pre_start initializes the component, handle() dispatches messages
//! to the component's public API, and supervision handles recovery.

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};

use crate::msg::BridgeMsg;

// ── Shared message type for all component actors ─────────────────────

#[allow(dead_code)]
pub enum ComponentMsg {
    /// Periodic tick — poll, flush, or heartbeat.
    Tick,
    /// Process an inbound signal.
    Signal { payload_sexp: String },
    /// Graceful shutdown.
    Shutdown,
}

// ── ChronicleActor ───────────────────────────────────────────────────

pub struct ChronicleActor;

impl Actor for ChronicleActor {
    type Msg = ComponentMsg;
    type State = bool; // initialized
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_chronicle::init();
        eprintln!("[INFO] [runtime] ChronicleActor started");
        Ok(true)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let _ = harmonia_chronicle::gc();
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ChronicleActor shutting down");
            }
            _ => {}
        }
        Ok(())
    }
}

// ── TailnetActor ─────────────────────────────────────────────────────

pub struct TailnetActor;

pub struct TailnetState {
    bridge: ActorRef<BridgeMsg>,
}

impl Actor for TailnetActor {
    type Msg = ComponentMsg;
    type State = TailnetState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_tailnet::transport::start_listener();
        eprintln!("[INFO] [runtime] TailnetActor started");
        Ok(TailnetState { bridge })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let messages = harmonia_tailnet::transport::poll_messages();
                for mesh_msg in messages {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Tailnet,
                        timestamp: now_unix(),
                        payload: MessagePayload::MeshInbound {
                            from_node: mesh_msg.from.to_string(),
                            msg_type: format!("{:?}", mesh_msg.msg_type),
                            payload: mesh_msg.payload,
                        },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Signal { payload_sexp } => {
                // Parse and send mesh message (best-effort)
                let _ = payload_sexp; // TODO: parse destination and message from sexp
            }
            ComponentMsg::Shutdown => {
                harmonia_tailnet::transport::stop_listener();
                eprintln!("[INFO] [runtime] TailnetActor shutting down");
            }
        }
        Ok(())
    }
}

// ── SignalogradActor ─────────────────────────────────────────────────
//
// Signalograd exposes C-style functions (from legacy FFI) that now
// compile as regular Rust pub fn. We wrap them with safe helpers.

pub struct SignalogradActor;

pub struct SignalogradState {
    bridge: ActorRef<BridgeMsg>,
}

impl Actor for SignalogradActor {
    type Msg = ComponentMsg;
    type State = SignalogradState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        harmonia_signalograd::harmonia_signalograd_init();
        eprintln!("[INFO] [runtime] SignalogradActor started");
        Ok(SignalogradState { bridge })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let status_ptr = harmonia_signalograd::harmonia_signalograd_status();
                if !status_ptr.is_null() {
                    let status = unsafe { std::ffi::CStr::from_ptr(status_ptr) }.to_string_lossy();
                    if !status.is_empty() && status != "()" {
                        let msg = HarmoniaMessage {
                            id: 0,
                            source: 0,
                            target: 0,
                            kind: ActorKind::Signalograd,
                            timestamp: now_unix(),
                            payload: MessagePayload::StateChanged {
                                to: status.into_owned(),
                            },
                        };
                        let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                    }
                    harmonia_signalograd::harmonia_signalograd_free_string(status_ptr);
                }
            }
            ComponentMsg::Signal { payload_sexp } => {
                let c_str = std::ffi::CString::new(payload_sexp).unwrap_or_default();
                harmonia_signalograd::harmonia_signalograd_observe(c_str.as_ptr());
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] SignalogradActor shutting down");
            }
        }
        Ok(())
    }
}

// ── ObservabilityActor ───────────────────────────────────────────────

pub struct ObservabilityActor;

impl Actor for ObservabilityActor {
    type Msg = ComponentMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] ObservabilityActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // Observability batches are flushed by the LangSmithClient internally
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ObservabilityActor shutting down");
            }
            _ => {}
        }
        Ok(())
    }
}

// ── GatewayActor ─────────────────────────────────────────────────────

pub struct GatewayActor;

pub struct GatewayState {
    bridge: ActorRef<BridgeMsg>,
}

impl Actor for GatewayActor {
    type Msg = ComponentMsg;
    type State = GatewayState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] GatewayActor started");
        Ok(GatewayState { bridge })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // Gateway polling: collect inbound signals from all frontends
                let registry = harmonia_gateway::Registry::new();
                let batch = harmonia_gateway::poll_baseband(&registry);
                for envelope in &batch.envelopes {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Gateway,
                        timestamp: now_unix(),
                        payload: MessagePayload::InboundSignal {
                            envelope_sexp: envelope.to_sexp(),
                        },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Signal { .. } => {
                // Outbound signals handled via direct gateway API from SBCL
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] GatewayActor shutting down");
            }
        }
        Ok(())
    }
}
