//! Proactive waker — periodic heartbeat publisher.
//!
//! On a configurable cadence the waker emits a small JSON heartbeat to the
//! agent's MQTT outbox topic. Mobile clients subscribed to the topic
//! observe the agent's liveness; downstream Phase 6b will extend this
//! actor to also call the marketplace `wake-device` endpoint when a
//! trusted client has missed heartbeats, so the broker can hand the
//! cluster's queued QoS-1 message off the moment the device wakes.
//!
//! Lives in `runtime/src/actors/` like the other component actors and is
//! linked to the runtime supervisor — restarted on crash with the same
//! frontend-registry handle.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_frontend_trait::FrontendMsg;

use crate::actors::ComponentMsg;
use crate::frontend_registry::FrontendRegistry;

pub struct ProactiveWakerActor;

pub struct ProactiveWakerState {
    pub frontend_registry: FrontendRegistry,
    pub agent_fp: String,
    pub seq: u64,
}

impl Actor for ProactiveWakerActor {
    type Msg = ComponentMsg;
    type State = ProactiveWakerState;
    type Arguments = (FrontendRegistry, String);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (frontend_registry, agent_fp): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [proactive-waker] started (agent_fp={agent_fp})");
        Ok(ProactiveWakerState {
            frontend_registry,
            agent_fp,
            seq: 0,
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
                state.seq = state.seq.wrapping_add(1);
                emit_heartbeat(state).await;
            }
            ComponentMsg::Dispatch(_, reply) => {
                // No external dispatch ops — this actor only ticks.
                let _ = reply.send("(:ok)".to_string());
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [proactive-waker] shutting down");
            }
        }
        Ok(())
    }
}

async fn emit_heartbeat(state: &ProactiveWakerState) {
    if state.agent_fp.is_empty() {
        // Agent fingerprint not provisioned yet — Phase 8 (pot provisioning)
        // writes it into vault. Until then the waker stays quiet.
        return;
    }
    let Some(entry) = state.frontend_registry.get("mqtt") else {
        // MQTT not yet up (unconfigured or still starting). Skip silently —
        // the next tick will retry.
        return;
    };
    let payload = json!({
        "kind": "heartbeat",
        "agent_fp": state.agent_fp,
        "seq": state.seq,
        "ts": chrono_secs(),
    })
    .to_string();
    // Channel "" routes through the MQTT actor's `send` impl which builds
    // the topic `harmonia/{agent_fp}/outbox/{channel}`. For the heartbeat
    // topic itself we use the dedicated address `__heartbeat__` and the
    // MQTT actor maps that to `harmonia/{agent_fp}/heartbeat`.
    if let Err(e) = ractor::call_t!(
        entry.actor,
        FrontendMsg::Send,
        2_000,
        "__heartbeat__".to_string(),
        payload
    ) {
        eprintln!("[WARN] [proactive-waker] heartbeat send rpc error: {e}");
    }
}

fn chrono_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
