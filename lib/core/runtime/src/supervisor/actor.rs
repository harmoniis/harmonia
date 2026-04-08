use std::collections::HashMap;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use harmonia_actor_protocol::{
    now_unix, ActorKind, ActorRegistration, ActorState, HarmoniaMessage, MessagePayload,
};

use crate::actors::ComponentMsg;
use crate::msg::{BridgeMsg, RuntimeMsg};
use crate::registry::ModuleEntry;

use super::restart;
use super::state::RuntimeState;

/// RuntimeSupervisor owns the actor registry and the SbclBridgeActor.
///
/// It replaces the in-process `ActorRegistry` + `RwLock` with a proper
/// ractor actor -- all registry mutations are serialized through the mailbox,
/// no locks needed.
pub struct RuntimeSupervisor;

impl Actor for RuntimeSupervisor {
    type Msg = RuntimeMsg;
    type State = RuntimeState;
    type Arguments = (ActorRef<BridgeMsg>, HashMap<String, ModuleEntry>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, module_registry): (ActorRef<BridgeMsg>, HashMap<String, ModuleEntry>),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] RuntimeSupervisor started");
        Ok(RuntimeState::new(bridge, module_registry))
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            RuntimeMsg::Register(kind, reply) => {
                let id = state.alloc_id();
                let now = now_unix();
                state.actors.insert(
                    id,
                    ActorRegistration {
                        id,
                        kind: kind.clone(),
                        state: ActorState::Starting,
                        registered_at: now,
                        last_heartbeat: now,
                        stall_ticks: 0,
                        message_count: 0,
                    },
                );
                eprintln!(
                    "[INFO] [runtime] Registered actor id={id} kind={}",
                    kind.as_str()
                );
                let _ = reply.send(id);
            }

            RuntimeMsg::Deregister(id, reply) => {
                let existed = state.actors.remove(&id).is_some();
                if existed {
                    eprintln!("[INFO] [runtime] Deregistered actor id={id}");
                }
                let _ = reply.send(existed);
            }

            RuntimeMsg::Post {
                source,
                target,
                payload_sexp,
            } => {
                let kind = state
                    .actors
                    .get(&source)
                    .map(|r| r.kind.clone())
                    .unwrap_or(ActorKind::CliAgent);
                if let Some(reg) = state.actors.get_mut(&source) {
                    reg.message_count += 1;
                }
                let msg = HarmoniaMessage {
                    id: state.alloc_msg_id(),
                    source,
                    target,
                    kind,
                    timestamp: now_unix(),
                    payload: MessagePayload::StateChanged { to: payload_sexp },
                };
                let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
            }

            RuntimeMsg::Heartbeat { id, bytes_delta } => {
                let kind = if let Some(reg) = state.actors.get_mut(&id) {
                    reg.last_heartbeat = now_unix();
                    reg.stall_ticks = 0;
                    reg.state = ActorState::Running;
                    if bytes_delta > 0 {
                        Some(reg.kind.clone())
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(kind) = kind {
                    let msg = HarmoniaMessage {
                        id: state.alloc_msg_id(),
                        source: id,
                        target: 0,
                        kind,
                        timestamp: now_unix(),
                        payload: MessagePayload::ProgressHeartbeat { bytes_delta },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }

            RuntimeMsg::DrainSbcl(reply) => {
                match ractor::call_t!(state.bridge, BridgeMsg::Drain, 5000) {
                    Ok(sexp) => {
                        let _ = reply.send(sexp);
                    }
                    Err(_) => {
                        let _ = reply.send("(:error \"drain timeout\")".to_string());
                    }
                }
            }

            RuntimeMsg::GetState(id, reply) => {
                let sexp = state.actor_state_sexp(id);
                let _ = reply.send(sexp);
            }

            RuntimeMsg::ListAll(reply) => {
                let sexp = state.list_sexp();
                let _ = reply.send(sexp);
            }

            RuntimeMsg::ComponentCall(component, sexp, reply) => {
                // The supervisor ROUTES, it never EXECUTES component logic.
                // Observability: dispatch directly (obs actor uses ObsMsg, not ComponentMsg).
                // Matrix: dispatch through actor for serialized access.
                // Others: route through component actor mailbox.
                if component == "observability" {
                    let result = crate::dispatch::dispatch("observability", &sexp);
                    let _ = reply.send(result);
                } else if component == "harmonic-matrix" {
                    if let Some(ref matrix) = state.matrix_actor {
                        let result = crate::dispatch::dispatch_matrix_via_actor(
                            matrix,
                            &state.obs_actor,
                            &sexp,
                        )
                        .await;
                        let _ = reply.send(result);
                    } else {
                        let _ = reply.send("(:error \"matrix actor not available\")".to_string());
                    }
                } else if let Some(actor) = state.component_actors.get(&component) {
                    // Route to component actor -- dispatch runs in its mailbox, not ours
                    match ractor::call_t!(actor, ComponentMsg::Dispatch, 30_000, sexp) {
                        Ok(result) => {
                            let _ = reply.send(result);
                        }
                        Err(_) => {
                            let _ = reply.send(format!(
                                "(:error \"component '{}' dispatch timeout\")",
                                component
                            ));
                        }
                    }
                } else {
                    let _ = reply.send(format!("(:error \"unknown component '{}'\")", component));
                }
            }

            RuntimeMsg::RegisterComponent(name, actor_ref) => {
                eprintln!(
                    "[INFO] [runtime] Registered component actor '{}' (ractor id={})",
                    name,
                    actor_ref.get_id()
                );
                state.component_actors.insert(name, actor_ref);
            }

            RuntimeMsg::RegisterMatrixActor(actor_ref) => {
                eprintln!(
                    "[INFO] [runtime] Registered matrix actor (ractor id={})",
                    actor_ref.get_id()
                );
                state.matrix_actor = Some(actor_ref);
            }

            RuntimeMsg::RegisterObsActor(actor_ref) => {
                eprintln!(
                    "[INFO] [runtime] Registered observability actor (ractor id={})",
                    actor_ref.get_id()
                );
                state.obs_actor = Some(actor_ref);
            }

            RuntimeMsg::SetDynamicRegistry(registry) => {
                state.dynamic_registry = Some(registry);
                eprintln!("[INFO] [runtime] DynamicRegistry injected into supervisor");
            }
            RuntimeMsg::SetTopicBus(bus) => {
                state.topic_bus = Some(bus);
                eprintln!("[INFO] [runtime] TopicBus injected into supervisor");
            }

            RuntimeMsg::ListModules(reply) => {
                let sexp = state.modules_list_sexp();
                let _ = reply.send(sexp);
            }

            RuntimeMsg::LoadModule(name, reply) => {
                let result = state.load_module(&name);
                let _ = reply.send(result);
            }

            RuntimeMsg::UnloadModule(name, reply) => {
                let result = state.unload_module(&name);
                let _ = reply.send(result);
            }

            RuntimeMsg::Shutdown => {
                eprintln!("[INFO] [runtime] Shutdown requested");
                state.shutting_down = true;
                // Two-phase shutdown: drain the bridge first to avoid losing messages
                match ractor::call_t!(state.bridge, BridgeMsg::Drain, 5000) {
                    Ok(drained) => {
                        eprintln!(
                            "[INFO] [runtime] Bridge drained before shutdown: {} bytes",
                            drained.len()
                        );
                    }
                    Err(e) => {
                        eprintln!("[WARN] [runtime] Bridge drain failed during shutdown: {e}");
                    }
                }
                state.bridge.stop(Some("shutdown".to_string()));
                myself.stop(Some("shutdown".to_string()));
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        restart::handle_supervision_event(myself, message, state).await
    }
}
