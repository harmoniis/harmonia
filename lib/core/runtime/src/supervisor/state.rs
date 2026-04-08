use std::collections::HashMap;

use ractor::ActorRef;

use harmonia_actor_protocol::{ActorId, ActorRegistration};
use harmonia_observability::ObsMsg;

use crate::actors::{ComponentMsg, MatrixMsg};
use crate::msg::BridgeMsg;
use crate::registry::{self, ModuleEntry, ModuleStatus};

/// Maximum number of restarts before giving up on an actor.
pub(super) const MAX_RESPAWNS: u32 = 5;

pub struct RuntimeState {
    pub(super) actors: HashMap<ActorId, ActorRegistration>,
    pub(super) next_id: u64,
    pub(super) next_msg_id: u64,
    pub(super) bridge: ActorRef<BridgeMsg>,
    pub(super) shutting_down: bool,
    /// Component actors tracked for supervisor restart.
    pub(super) component_actors: HashMap<String, ActorRef<ComponentMsg>>,
    /// Matrix actor (separate message type).
    pub(super) matrix_actor: Option<ActorRef<MatrixMsg>>,
    /// Observability actor (ObsMsg, not ComponentMsg).
    pub(super) obs_actor: Option<ActorRef<ObsMsg>>,
    /// Module registry for runtime load/unload management.
    pub module_registry: HashMap<String, ModuleEntry>,
    /// Respawn counters for crash-loop prevention.
    pub(super) respawn_counts: HashMap<String, u32>,
    /// DynamicRegistry for re-registering actors after restart.
    pub(super) dynamic_registry: Option<crate::dynamic_registry::SharedDynamicRegistry>,
    /// TopicBus for unsubscribing crashed actors and re-subscribing after restart.
    pub(super) topic_bus: Option<crate::topic_bus::SharedTopicBus>,
}

impl RuntimeState {
    pub(super) fn new(
        bridge: ActorRef<BridgeMsg>,
        module_registry: HashMap<String, ModuleEntry>,
    ) -> Self {
        Self {
            actors: HashMap::new(),
            next_id: 1,
            next_msg_id: 1,
            bridge,
            shutting_down: false,
            component_actors: HashMap::new(),
            matrix_actor: None,
            obs_actor: None,
            module_registry,
            respawn_counts: HashMap::new(),
            dynamic_registry: None,
            topic_bus: None,
        }
    }

    pub(super) fn alloc_id(&mut self) -> ActorId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(super) fn alloc_msg_id(&mut self) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        id
    }

    pub(super) fn list_sexp(&self) -> String {
        let mut entries: Vec<String> = self.actors.values()
            .map(|reg| format!("(:id {} :kind :{} :state :{})",
                reg.id, reg.kind.as_str(), reg.state.as_str()))
            .collect();
        // Include DynamicRegistry component list with capabilities.
        if let Some(reg) = &self.dynamic_registry {
            let components = reg.components();
            for name in &components {
                let caps = reg.capabilities_of(name);
                let caps_sexp = caps.iter()
                    .map(|c| format!("\"{}\"", c))
                    .collect::<Vec<_>>().join(" ");
                entries.push(format!("(:component \"{}\" :capabilities ({}))", name, caps_sexp));
            }
        }
        entries.sort();
        format!("({})", entries.join(" "))
    }

    pub(super) fn actor_state_sexp(&self, id: ActorId) -> String {
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

    pub(super) fn modules_list_sexp(&self) -> String {
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

    pub(super) fn load_module(&mut self, name: &str) -> String {
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

    pub(super) fn unload_module(&mut self, name: &str) -> String {
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
