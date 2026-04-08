//! GatewayActor — frontend signal ingestion.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_observability::{ObsMsg, Traceable};

use crate::msg::BridgeMsg;
use super::ComponentMsg;

pub struct GatewayActor;

pub struct GatewayState {
    bridge: ActorRef<BridgeMsg>,
    obs: Option<ActorRef<ObsMsg>>,
}

impl Actor for GatewayActor {
    type Msg = ComponentMsg;
    type State = GatewayState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] GatewayActor started");
        Ok(GatewayState { bridge, obs })
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
                if harmonia_observability::harmonia_observability_is_verbose()
                    && !batch.envelopes.is_empty()
                {
                    state.obs.trace_event(
                        "gateway-poll",
                        "tool",
                        json!({"envelopes": batch.envelopes.len()}),
                    );
                }
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
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("gateway", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] GatewayActor shutting down");
            }
        }
        Ok(())
    }
}
