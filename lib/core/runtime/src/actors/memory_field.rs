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
    // ── Automated dreaming (Phase 4A) ──
    idle_ticks: u64,
    last_dream_cycle: u64,
    total_ticks: u64,
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
            idle_ticks: 0,
            last_dream_cycle: 0,
            total_ticks: 0,
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
                state.total_ticks += 1;
                state.idle_ticks += 1;

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

                // ── Automated dreaming (Phase 4A) ──
                // When idle long enough and enough ticks since last dream, trigger a dream cycle.
                let dream_idle_threshold = harmonia_config_store::get_own("memory-field", "dream-idle-ticks")
                    .ok().flatten().and_then(|s| s.parse::<u64>().ok()).unwrap_or(5);
                let dream_cycle_interval = harmonia_config_store::get_own("memory-field", "dream-cycle-interval")
                    .ok().flatten().and_then(|s| s.parse::<u64>().ok()).unwrap_or(30);

                if state.idle_ticks >= dream_idle_threshold
                    && state.total_ticks - state.last_dream_cycle >= dream_cycle_interval
                {
                    if let Ok(report) = harmonia_memory_field::field_dream(&mut state.field) {
                        let report_sexp = harmonia_memory_field::dream_report_to_sexp(&report);
                        // Send dream report to Lisp via bridge for merge application.
                        let msg = HarmoniaMessage {
                            id: 0,
                            source: 0,
                            target: 0,
                            kind: ActorKind::MemoryField,
                            timestamp: now_unix(),
                            payload: MessagePayload::InboundSignal {
                                envelope_sexp: format!(
                                    "(:component \"memory-field\" :event \"dream-complete\" :report {})",
                                    report_sexp
                                ),
                            },
                        };
                        let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                    }
                    state.last_dream_cycle = state.total_ticks;
                    state.idle_ticks = 0;
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                state.idle_ticks = 0; // Activity resets idle counter.
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
