//! MemoryFieldActor — attractor basin monitoring.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_observability::{ObsMsg, Traceable};

use crate::msg::BridgeMsg;
use super::ComponentMsg;

pub struct MemoryFieldActor;

pub struct MemoryFieldState {
    bridge: ActorRef<BridgeMsg>,
    obs: Option<ActorRef<ObsMsg>>,
    field: harmonia_memory_field::FieldState,
    last_basin: String,
}

impl Actor for MemoryFieldActor {
    type Msg = ComponentMsg;
    type State = MemoryFieldState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let field = harmonia_memory_field::FieldState::new();
        eprintln!("[INFO] [runtime] MemoryFieldActor started");
        Ok(MemoryFieldState {
            bridge,
            obs,
            field,
            last_basin: "thomas-0".into(),
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
                // Attractors are stepped by harmonic machine via IPC (:attractor-sync phase).
                // Tick monitors basin transitions and emits StateChanged through bridge.
                if let Ok(status_sexp) = harmonia_memory_field::basin_status(&state.field) {
                    if let Some(basin) = extract_basin_from_sexp(&status_sexp) {
                        if basin != state.last_basin {
                            // Basin switched — emit StateChanged to bridge for Lisp consumption.
                            let msg = HarmoniaMessage {
                                id: 0,
                                source: 0,
                                target: 0,
                                kind: ActorKind::MemoryField,
                                timestamp: now_unix(),
                                payload: MessagePayload::StateChanged {
                                    to: format!("basin:{}", basin),
                                },
                            };
                            let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                            // Observability trace for basin transition.
                            if let Some(obs) = &state.obs {
                                if harmonia_observability::harmonia_observability_is_standard() {
                                    let obs_opt: Option<ActorRef<ObsMsg>> = Some(obs.clone());
                                    obs_opt.trace_event(
                                        "memory-field-basin-switch",
                                        "chain",
                                        json!({
                                            "from": state.last_basin,
                                            "to": basin
                                        }),
                                    );
                                }
                            }
                            state.last_basin = basin;
                        }
                    }
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result =
                    crate::dispatch::dispatch_memory_field(&sexp, &mut state.field);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] MemoryFieldActor shutting down");
            }
        }
        Ok(())
    }
}

/// Extract basin string from sexp like "(:ok :current :thomas-2 ...)"
pub(crate) fn extract_basin_from_sexp(sexp: &str) -> Option<String> {
    if let Some(pos) = sexp.find(":current ") {
        let rest = &sexp[pos + 9..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(rest.len());
        let basin = rest[..end].trim();
        if !basin.is_empty() {
            return Some(basin.to_string());
        }
    }
    None
}
