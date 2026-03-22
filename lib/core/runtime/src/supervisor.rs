use std::collections::HashMap;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use harmonia_actor_protocol::{
    now_unix, ActorId, ActorKind, ActorRegistration, ActorState, HarmoniaMessage, MessagePayload,
};

use crate::actors::{ComponentMsg, MatrixMsg};
use crate::msg::{BridgeMsg, RuntimeMsg};
use crate::registry::{self, ModuleEntry, ModuleStatus};

/// RuntimeSupervisor owns the actor registry and the SbclBridgeActor.
///
/// It replaces the in-process `ActorRegistry` + `RwLock` with a proper
/// ractor actor — all registry mutations are serialized through the mailbox,
/// no locks needed.
pub struct RuntimeSupervisor;

/// Maximum number of restarts before giving up on an actor.
const MAX_RESPAWNS: u32 = 5;

pub struct RuntimeState {
    actors: HashMap<ActorId, ActorRegistration>,
    next_id: u64,
    next_msg_id: u64,
    bridge: ActorRef<BridgeMsg>,
    shutting_down: bool,
    /// Component actors tracked for supervisor restart.
    component_actors: HashMap<String, ActorRef<ComponentMsg>>,
    /// Matrix actor (separate message type).
    matrix_actor: Option<ActorRef<MatrixMsg>>,
    /// Module registry for runtime load/unload management.
    pub module_registry: HashMap<String, ModuleEntry>,
    /// Respawn counters for crash-loop prevention.
    respawn_counts: HashMap<String, u32>,
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

    fn modules_list_sexp(&self) -> String {
        let mut entries: Vec<String> = Vec::new();
        let mut names: Vec<&String> = self.module_registry.keys().collect();
        names.sort();
        for name in &names {
            if let Some(entry) = self.module_registry.get(*name) {
                let status_str = match &entry.status {
                    ModuleStatus::Unloaded => "unloaded".to_string(),
                    ModuleStatus::Loaded => "loaded".to_string(),
                    ModuleStatus::Error(e) => {
                        format!("error \"{}\"", harmonia_actor_protocol::sexp_escape(e))
                    }
                };
                let core_str = if entry.core { " :core t" } else { "" };
                let needs: Vec<String> =
                    entry.config_reqs.iter().map(|r| format!("{}", r)).collect();
                let needs_str = if needs.is_empty() {
                    String::new()
                } else {
                    format!(
                        " :needs \"{}\"",
                        harmonia_actor_protocol::sexp_escape(&needs.join(", "))
                    )
                };
                entries.push(format!(
                    "(:name \"{}\" :status {}{}{})",
                    name, status_str, core_str, needs_str
                ));
            }
        }
        format!("({})", entries.join(" "))
    }

    fn load_module(&mut self, name: &str) -> String {
        let esc_name = harmonia_actor_protocol::sexp_escape(name);
        let entry = match self.module_registry.get_mut(name) {
            Some(e) => e,
            None => return format!("(:error \"unknown module '{}'\")", esc_name),
        };

        if matches!(entry.status, ModuleStatus::Loaded) {
            return format!("(:ok \"module '{}' is already loaded\")", esc_name);
        }

        // Validate config requirements
        if let Err(e) = registry::validate_config(&entry.config_reqs) {
            entry.status = ModuleStatus::Error(e.clone());
            return format!("(:error \"{}\")", harmonia_actor_protocol::sexp_escape(&e));
        }

        // Call init
        match (entry.init_fn)() {
            Ok(()) => {
                entry.status = ModuleStatus::Loaded;
                eprintln!("[INFO] [runtime] Module '{}' loaded", name);
                format!("(:ok \"module '{}' loaded\")", esc_name)
            }
            Err(e) => {
                entry.status = ModuleStatus::Error(e.clone());
                eprintln!("[WARN] [runtime] Module '{}' failed to load: {}", name, e);
                format!("(:error \"{}\")", harmonia_actor_protocol::sexp_escape(&e))
            }
        }
    }

    fn unload_module(&mut self, name: &str) -> String {
        let esc_name = harmonia_actor_protocol::sexp_escape(name);
        let entry = match self.module_registry.get_mut(name) {
            Some(e) => e,
            None => return format!("(:error \"unknown module '{}'\")", esc_name),
        };

        if entry.core {
            return format!("(:error \"cannot unload core module '{}'\")", esc_name);
        }

        if matches!(entry.status, ModuleStatus::Unloaded) {
            return format!("(:ok \"module '{}' is already unloaded\")", esc_name);
        }

        (entry.shutdown_fn)();
        entry.status = ModuleStatus::Unloaded;
        eprintln!("[INFO] [runtime] Module '{}' unloaded", name);
        format!("(:ok \"module '{}' unloaded\")", esc_name)
    }
}

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
        Ok(RuntimeState {
            actors: HashMap::new(),
            next_id: 1,
            next_msg_id: 1,
            bridge,
            shutting_down: false,
            component_actors: HashMap::new(),
            matrix_actor: None,
            module_registry,
            respawn_counts: HashMap::new(),
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

            RuntimeMsg::ComponentCall(component, sexp, reply) => {
                // The supervisor ROUTES, it never EXECUTES component logic.
                // Every component has an actor — dispatch runs in the actor's
                // mailbox, keeping the supervisor free for heartbeats/drains/shutdown.
                if component == "harmonic-matrix" {
                    if let Some(ref matrix) = state.matrix_actor {
                        let result =
                            crate::dispatch::dispatch_matrix_via_actor(matrix, &sexp).await;
                        let _ = reply.send(result);
                    } else {
                        let _ = reply.send("(:error \"matrix actor not available\")".to_string());
                    }
                } else if let Some(actor) = state.component_actors.get(&component) {
                    // Route to component actor — dispatch runs in its mailbox, not ours
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
        if state.shutting_down {
            // During shutdown we expect actors to terminate — don't restart anything.
            return Ok(());
        }

        let failed_id = match &message {
            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                eprintln!(
                    "[INFO] [runtime] Supervised actor terminated: id={}, reason={reason:?}",
                    cell.get_id()
                );
                cell.get_id()
            }
            SupervisionEvent::ActorFailed(cell, err) => {
                eprintln!(
                    "[WARN] [runtime] Supervised actor failed: id={}, err={err}",
                    cell.get_id()
                );
                cell.get_id()
            }
            _ => return Ok(()),
        };

        // Try to restart the bridge actor
        let bridge_id = state.bridge.get_id();
        if failed_id == bridge_id {
            let count = state
                .respawn_counts
                .entry("sbcl-bridge".to_string())
                .or_insert(0);
            *count += 1;
            if *count > MAX_RESPAWNS {
                eprintln!("[ERROR] [runtime] SbclBridgeActor exceeded max respawns ({MAX_RESPAWNS}), giving up");
                return Ok(());
            }
            eprintln!("[INFO] [runtime] Respawning SbclBridgeActor ({count}/{MAX_RESPAWNS})");
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
                    eprintln!("[INFO] [runtime] SbclBridgeActor respawned successfully");
                }
                Err(e) => {
                    eprintln!("[ERROR] [runtime] Failed to respawn SbclBridgeActor: {e}");
                }
            }
            return Ok(());
        }

        // Try to restart a component actor
        // Find the component name whose ActorRef matches the failed ractor id
        let component_name = state
            .component_actors
            .iter()
            .find(|(_, r)| r.get_id() == failed_id)
            .map(|(name, _)| name.clone());

        if let Some(name) = component_name {
            let count = state.respawn_counts.entry(name.clone()).or_insert(0);
            *count += 1;
            if *count > MAX_RESPAWNS {
                eprintln!(
                    "[ERROR] [runtime] Component '{name}' exceeded max respawns ({MAX_RESPAWNS}), giving up"
                );
                state.component_actors.remove(&name);
                return Ok(());
            }
            eprintln!(
                "[INFO] [runtime] Respawning component actor '{name}' ({count}/{MAX_RESPAWNS})"
            );
            let spawn_result = match name.as_str() {
                "chronicle" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::ChronicleActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "gateway" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::GatewayActor,
                    state.bridge.clone(),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "tailnet" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::TailnetActor,
                    state.bridge.clone(),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "signalograd" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::SignalogradActor,
                    state.bridge.clone(),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "observability" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::ObservabilityActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "vault" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::VaultActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "config" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::ConfigActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "provider-router" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::ProviderRouterActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                "parallel" => Actor::spawn_linked(
                    Some(name.clone()),
                    crate::actors::ParallelActor,
                    (),
                    myself.get_cell(),
                )
                .await
                .map(|(r, _)| r),
                // Note: "harmonic-matrix" uses MatrixMsg, handled separately below
                _ => {
                    eprintln!("[WARN] [runtime] Unknown component actor '{name}', cannot respawn");
                    return Ok(());
                }
            };

            match spawn_result {
                Ok(new_ref) => {
                    state.component_actors.insert(name.clone(), new_ref);
                    eprintln!("[INFO] [runtime] Component actor '{name}' respawned successfully");
                }
                Err(e) => {
                    eprintln!("[ERROR] [runtime] Failed to respawn component actor '{name}': {e}");
                }
            }
            return Ok(());
        }

        // Try to restart the matrix actor
        if let Some(ref matrix) = state.matrix_actor {
            if matrix.get_id() == failed_id {
                eprintln!("[INFO] [runtime] Respawning HarmonicMatrixActor after failure");
                match Actor::spawn_linked(
                    Some("harmonic-matrix".to_string()),
                    crate::actors::HarmonicMatrixActor,
                    (),
                    myself.get_cell(),
                )
                .await
                {
                    Ok((new_ref, _)) => {
                        state.matrix_actor = Some(new_ref);
                        eprintln!("[INFO] [runtime] HarmonicMatrixActor respawned successfully");
                    }
                    Err(e) => {
                        eprintln!("[ERROR] [runtime] Failed to respawn HarmonicMatrixActor: {e}");
                    }
                }
            }
        }

        Ok(())
    }
}
