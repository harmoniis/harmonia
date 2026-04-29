//! SignalogradActor — signal processing kernel.
//!
//! Actor owns KernelState directly. All dispatch goes through typed API.

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_observability::ObsMsg;

use crate::msg::BridgeMsg;
use super::ComponentMsg;

pub struct SignalogradActor;

pub struct SignalogradState {
    bridge: ActorRef<BridgeMsg>,
    kernel: harmonia_signalograd::KernelState,
}

impl Actor for SignalogradActor {
    type Msg = ComponentMsg;
    type State = SignalogradState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, _obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        // Load persisted state or start fresh.
        let kernel = harmonia_signalograd::checkpoint::load_state()
            .unwrap_or_else(|_| harmonia_signalograd::KernelState::new());
        eprintln!("[INFO] [runtime] SignalogradActor started");
        Ok(SignalogradState { bridge, kernel })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let proposal = harmonia_signalograd::projection_to_sexp(
                    &state.kernel.last_projection,
                );
                if !proposal.is_empty() {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Signalograd,
                        timestamp: now_unix(),
                        payload: MessagePayload::StateChanged { to: proposal },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result =
                    crate::dispatch::dispatch_signalograd(&sexp, &mut state.kernel);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] SignalogradActor shutting down");
            }
        }
        Ok(())
    }
}
