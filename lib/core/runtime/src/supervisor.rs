use std::collections::HashMap;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use harmonia_actor_protocol::{
    now_unix, ActorId, ActorKind, ActorRegistration, ActorState, HarmoniaMessage, MessagePayload,
};

use crate::msg::{BridgeMsg, RuntimeMsg};

/// RuntimeSupervisor owns the actor registry and the SbclBridgeActor.
///
/// It replaces the in-process `ActorRegistry` + `RwLock` with a proper
/// ractor actor — all registry mutations are serialized through the mailbox,
/// no locks needed.
pub struct RuntimeSupervisor;

pub struct RuntimeState {
    actors: HashMap<ActorId, ActorRegistration>,
    next_id: u64,
    next_msg_id: u64,
    bridge: ActorRef<BridgeMsg>,
    shutting_down: bool,
}

impl RuntimeState {
    fn alloc_id(&mut self) -> ActorId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn alloc_msg_id(&mut self) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        id
    }

    fn list_sexp(&self) -> String {
        if self.actors.is_empty() {
            return "()".to_string();
        }
        let mut entries: Vec<String> = self
            .actors
            .values()
            .map(|reg| {
                format!(
                    "(:id {} :kind :{} :state :{})",
                    reg.id,
                    reg.kind.as_str(),
                    reg.state.as_str()
                )
            })
            .collect();
        entries.sort();
        format!("({})", entries.join(" "))
    }

    fn actor_state_sexp(&self, id: ActorId) -> String {
        match self.actors.get(&id) {
            Some(reg) => format!(
                "(:id {} :kind :{} :state :{} :registered-at {} :last-heartbeat {} :stall-ticks {} :message-count {})",
                reg.id,
                reg.kind.as_str(),
                reg.state.as_str(),
                reg.registered_at,
                reg.last_heartbeat,
                reg.stall_ticks,
                reg.message_count,
            ),
            None => format!("(:error \"actor {} not found\")", id),
        }
    }
}

impl Actor for RuntimeSupervisor {
    type Msg = RuntimeMsg;
    type State = RuntimeState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] RuntimeSupervisor started");
        Ok(RuntimeState {
            actors: HashMap::new(),
            next_id: 1,
            next_msg_id: 1,
            bridge,
            shutting_down: false,
        })
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
        match &message {
            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                eprintln!(
                    "[INFO] [runtime] Supervised actor terminated: id={}, reason={reason:?}",
                    cell.get_id()
                );
            }
            SupervisionEvent::ActorFailed(cell, err) => {
                eprintln!(
                    "[WARN] [runtime] Supervised actor failed: id={}, err={err}",
                    cell.get_id()
                );
                // Attempt to restart the bridge actor if we're not shutting down
                if !state.shutting_down {
                    let bridge_id = state.bridge.get_id();
                    if cell.get_id() == bridge_id {
                        eprintln!("[INFO] [runtime] Respawning SbclBridgeActor after failure");
                        match Actor::spawn_linked(
                            Some("sbcl-bridge".to_string()),
                            crate::bridge::SbclBridgeActor,
                            (),
                            myself.get_cell(),
                        )
                        .await
                        {
                            Ok((new_bridge, _)) => {
                                state.bridge = new_bridge;
                                eprintln!(
                                    "[INFO] [runtime] SbclBridgeActor respawned successfully"
                                );
                            }
                            Err(e) => {
                                eprintln!(
                                    "[ERROR] [runtime] Failed to respawn SbclBridgeActor: {e}"
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
