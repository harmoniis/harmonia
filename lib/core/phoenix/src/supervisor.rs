use std::collections::HashMap;
use std::time::Instant;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};

use crate::config::{PhoenixConfig, SubsystemConfig};
use crate::msg::*;
use crate::subsystem::SubsystemActor;
use crate::trauma;

pub struct PhoenixSupervisor;

struct ChildEntry {
    actor_ref: ActorRef<SubsystemMsg>,
    config: SubsystemConfig,
    state: SubsystemState,
    core: bool,
    /// Restart counter persisted across SubsystemActor incarnations so a
    /// crashing-and-respawning child can never exceed `max_restarts`.
    restart_count: u32,
}

pub struct SupervisorState {
    children: HashMap<String, ChildEntry>,
    start_time: Instant,
    shutting_down: bool,
    shutdown_timeout_secs: u64,
}

/// Set when the supervisor initiates shutdown because a core subsystem
/// permanently failed. `main` reads this after the supervisor exits so the
/// process can exit with a non-zero code, prompting service managers
/// (launchd / systemd / rc.d) to back off rather than spin in a tight loop.
pub static FATAL_CORE_FAILURE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

impl Actor for PhoenixSupervisor {
    type Msg = SupervisorMsg;
    type State = SupervisorState;
    type Arguments = PhoenixConfig;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        config: PhoenixConfig,
    ) -> Result<Self::State, ActorProcessingErr> {
        let mut children = HashMap::new();

        for sub_cfg in &config.subsystems {
            match spawn_child(&myself, sub_cfg, 0).await {
                Ok(actor_ref) => {
                    children.insert(
                        sub_cfg.name.clone(),
                        ChildEntry {
                            actor_ref,
                            config: sub_cfg.clone(),
                            state: SubsystemState::Starting,
                            core: sub_cfg.core,
                            restart_count: 0,
                        },
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[ERROR] [phoenix] Failed to spawn actor for subsystem={}: {e}",
                        sub_cfg.name
                    );
                    trauma::append_trauma(&format!(
                        "actor-spawn-failed subsystem={}: {e}",
                        sub_cfg.name
                    ));
                }
            }
        }

        let mode = derive_mode(&children);
        eprintln!(
            "[INFO] [phoenix] Supervisor started, mode={mode:?}, children={}",
            children.len()
        );

        Ok(SupervisorState {
            children,
            start_time: Instant::now(),
            shutting_down: false,
            shutdown_timeout_secs: config.shutdown_timeout_secs,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisorMsg::GetHealth(reply) => {
                let snapshot = build_snapshot(state);
                let _ = reply.send(snapshot);
            }
            SupervisorMsg::SubsystemStateChanged {
                name,
                state: new_state,
            } => {
                if state.children.contains_key(&name) {
                    let old_mode = derive_mode(&state.children);
                    let entry = state.children.get_mut(&name).unwrap();
                    let became_failed = matches!(new_state, SubsystemState::Failed { .. });
                    let was_core = entry.core;
                    entry.state = new_state;
                    let new_mode = derive_mode(&state.children);
                    if old_mode != new_mode {
                        eprintln!("[INFO] [phoenix] Mode changed: {old_mode:?} → {new_mode:?}");
                    }

                    // A core subsystem reaching `Failed` (max_restarts exceeded
                    // or unrecoverable spawn failure) is a fatal condition: the
                    // supervisor exits non-zero so launchd / systemd / rc.d can
                    // back off instead of us spinning silently in a degraded
                    // mode. This is the single most important difference vs.
                    // the previous behaviour, where the supervisor stayed up
                    // forever next to a permanently-broken child.
                    if became_failed && was_core && !state.shutting_down {
                        eprintln!(
                            "[ERROR] [phoenix] Core subsystem {name} permanently failed — initiating shutdown"
                        );
                        trauma::append_trauma(&format!(
                            "core-subsystem-failed subsystem={name} initiating-shutdown"
                        ));
                        FATAL_CORE_FAILURE.store(true, std::sync::atomic::Ordering::SeqCst);
                        let _ = myself.cast(SupervisorMsg::Shutdown);
                    }
                }
            }
            SupervisorMsg::SubsystemRestartCount { name, count } => {
                if let Some(entry) = state.children.get_mut(&name) {
                    entry.restart_count = count;
                }
            }
            SupervisorMsg::Shutdown => {
                eprintln!("[INFO] [phoenix] Shutdown requested, stopping all children");
                state.shutting_down = true;
                trauma::chronicle_record("shutdown", None, None, None, None);

                let timeout = state.shutdown_timeout_secs;
                for entry in state.children.values() {
                    let _ = entry.actor_ref.cast(SubsystemMsg::Stop {
                        timeout_secs: timeout,
                    });
                }

                if state.children.is_empty() {
                    myself.stop(Some("shutdown-complete".to_string()));
                }

                // Safety net: if children don't stop in time, force exit
                let sup_ref = myself.clone();
                let n = state.children.len();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(timeout + 5)).await;
                    eprintln!(
                        "[WARN] [phoenix] Shutdown timeout, forcing exit ({n} children remaining)"
                    );
                    sup_ref.stop(Some("shutdown-timeout".to_string()));
                });
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
        let cell_id = match &message {
            SupervisionEvent::ActorTerminated(cell, _, _) => cell.get_id(),
            SupervisionEvent::ActorFailed(cell, _) => cell.get_id(),
            _ => return Ok(()),
        };

        // Find which child this was
        let child_name = state
            .children
            .iter()
            .find(|(_, entry)| entry.actor_ref.get_id() == cell_id)
            .map(|(name, _)| name.clone());

        let Some(name) = child_name else {
            return Ok(());
        };

        if state.shutting_down {
            state.children.remove(&name);
            eprintln!(
                "[INFO] [phoenix] Child actor {name} stopped during shutdown ({} remaining)",
                state.children.len()
            );
            if state.children.is_empty() {
                eprintln!("[INFO] [phoenix] All children stopped, supervisor exiting");
                myself.stop(Some("shutdown-complete".to_string()));
            }
            return Ok(());
        }

        // The SubsystemActor itself crashed (not the OS process — that's handled
        // inside the actor). Respawn the actor to recover.
        let is_failure = matches!(message, SupervisionEvent::ActorFailed(_, _));
        if is_failure {
            eprintln!("[WARN] [phoenix] Child actor {name} crashed, respawning actor");
        } else {
            eprintln!("[WARN] [phoenix] Child actor {name} terminated unexpectedly, respawning");
        }

        let Some(child) = state.children.get(&name) else {
            eprintln!("[WARN] [phoenix] Child {name} vanished during respawn");
            return Ok(());
        };
        let config = child.config.clone();
        let restart_count = child.restart_count;

        match spawn_child(&myself, &config, restart_count).await {
            Ok(actor_ref) => {
                if let Some(entry) = state.children.get_mut(&name) {
                    entry.actor_ref = actor_ref;
                    entry.state = SubsystemState::Starting;
                    eprintln!("[INFO] [phoenix] Respawned actor for subsystem={name}");
                }
            }
            Err(e) => {
                eprintln!("[ERROR] [phoenix] Failed to respawn actor for {name}: {e}");
                trauma::append_trauma(&format!("actor-respawn-failed subsystem={name}: {e}"));
                if let Some(entry) = state.children.get_mut(&name) {
                    entry.state = SubsystemState::Failed {
                        reason: format!("actor crash, respawn failed: {e}"),
                        attempts: 0,
                    };
                }
            }
        }

        Ok(())
    }
}

async fn spawn_child(
    supervisor: &ActorRef<SupervisorMsg>,
    config: &SubsystemConfig,
    initial_restart_count: u32,
) -> Result<ActorRef<SubsystemMsg>, Box<dyn std::error::Error>> {
    let (actor_ref, _handle) = Actor::spawn_linked(
        Some(config.name.clone()),
        SubsystemActor,
        (config.clone(), supervisor.clone(), initial_restart_count),
        supervisor.get_cell(),
    )
    .await?;
    Ok(actor_ref)
}

fn derive_mode(children: &HashMap<String, ChildEntry>) -> DaemonMode {
    if children.is_empty() {
        return DaemonMode::Starting;
    }

    let any_core_failed = children.values().any(|c| c.core && c.state.is_failed());
    if any_core_failed {
        return DaemonMode::CoreOnly;
    }

    let failed_non_core: Vec<String> = children
        .iter()
        .filter(|(_, c)| !c.core && c.state.is_failed())
        .map(|(name, _)| name.clone())
        .collect();
    if !failed_non_core.is_empty() {
        return DaemonMode::Degraded {
            failed: failed_non_core,
        };
    }

    let any_starting = children.values().any(|c| c.state.is_starting_or_backoff());
    if any_starting {
        return DaemonMode::Starting;
    }

    // All Running or Stopped, none Starting, none Failed
    DaemonMode::Full
}

fn build_snapshot(state: &SupervisorState) -> HealthSnapshot {
    let subsystems: HashMap<String, SubsystemState> = state
        .children
        .iter()
        .map(|(name, entry)| {
            // Use cached state (kept up-to-date by SubsystemStateChanged messages).
            // GetState is available for on-demand verification via IPC.
            let sanitized = match &entry.state {
                SubsystemState::Running { .. } => SubsystemState::Running { pid: 0 },
                other => other.clone(),
            };
            (name.clone(), sanitized)
        })
        .collect();
    let mode = derive_mode(&state.children);
    HealthSnapshot {
        mode,
        uptime_secs: state.start_time.elapsed().as_secs(),
        subsystems,
    }
}

