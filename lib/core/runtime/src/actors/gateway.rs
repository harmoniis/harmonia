//! GatewayActor — frontend signal ingestion.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_gateway::SenderPolicyMsg;
use harmonia_observability::{ObsMsg, Traceable};
use harmonia_transport_pgp::TransportPgp;

use crate::msg::BridgeMsg;
use super::ComponentMsg;

pub struct GatewayActor;

pub struct GatewayState {
    bridge: ActorRef<BridgeMsg>,
    obs: Option<ActorRef<ObsMsg>>,
    /// Trait-based frontends live here. The dispatch path consults this
    /// before falling back to the legacy free-function frontends.
    frontend_registry: crate::frontend_registry::FrontendRegistry,
    /// Shared PGP authenticator. Used by `dispatch::gateway::poll_all_frontends`
    /// to verify inbound signed envelopes uniformly across MQTT/HTTP/Email
    /// before stamping `:auth-method` / `:auth-level` / `:auth-fp`.
    transport_pgp: TransportPgp,
    /// Sender-policy actor — owns the deny/allow cache. Inbound envelopes
    /// pass through `IsSignalAllowed` before they are forwarded to the bridge.
    sender_policy: ActorRef<SenderPolicyMsg>,
}

impl Actor for GatewayActor {
    type Msg = ComponentMsg;
    type State = GatewayState;
    type Arguments = (
        ActorRef<BridgeMsg>,
        Option<ActorRef<ObsMsg>>,
        crate::frontend_registry::FrontendRegistry,
        TransportPgp,
        ActorRef<SenderPolicyMsg>,
    );

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs, frontend_registry, transport_pgp, sender_policy): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] GatewayActor started");
        Ok(GatewayState {
            bridge,
            obs,
            frontend_registry,
            transport_pgp,
            sender_policy,
        })
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

                // Apply sender-policy: every envelope passes through the actor.
                // Fail-closed on timeout — a stalled policy actor must not let
                // unauthenticated messaging traffic through.
                let mut allowed = Vec::with_capacity(batch.envelopes.len());
                for envelope in batch.envelopes {
                    match ractor::call_t!(
                        state.sender_policy,
                        SenderPolicyMsg::IsSignalAllowed,
                        50,
                        envelope.clone()
                    ) {
                        Ok(true) => allowed.push(envelope),
                        Ok(false) => {}
                        Err(e) => {
                            eprintln!(
                                "[WARN] [runtime] sender-policy unavailable, dropping envelope: {e}"
                            );
                        }
                    }
                }

                if harmonia_observability::harmonia_observability_is_verbose()
                    && !allowed.is_empty()
                {
                    state.obs.trace_event(
                        "gateway-poll",
                        "tool",
                        json!({"envelopes": allowed.len()}),
                    );
                }
                for envelope in &allowed {
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
                let result = crate::dispatch::gateway::dispatch(
                    &sexp,
                    &state.frontend_registry,
                    &state.transport_pgp,
                )
                .await;
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] GatewayActor shutting down");
            }
        }
        Ok(())
    }
}
