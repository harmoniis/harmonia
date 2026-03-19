use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::process::Command;

use crate::config::SubsystemConfig;
use crate::msg::{RestartPolicy, SubsystemMsg, SubsystemState, SupervisorMsg};
use crate::trauma;

pub struct SubsystemActor;

pub struct SubsystemActorState {
    config: SubsystemConfig,
    supervisor: ActorRef<SupervisorMsg>,
    pid: Option<u32>,
    state: SubsystemState,
    restart_count: u32,
    stopping: bool,
}

impl SubsystemActorState {
    fn set_state(&mut self, new_state: SubsystemState) {
        self.state = new_state.clone();
        let _ = self.supervisor.cast(SupervisorMsg::SubsystemStateChanged {
            name: self.config.name.clone(),
            state: new_state,
        });
    }
}

impl Actor for SubsystemActor {
    type Msg = SubsystemMsg;
    type State = SubsystemActorState;
    type Arguments = (SubsystemConfig, ActorRef<SupervisorMsg>);

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (config, supervisor) = args;
        let delay = config.startup_delay_ms;
        let mut state = SubsystemActorState {
            config,
            supervisor,
            pid: None,
            state: SubsystemState::Stopped,
            restart_count: 0,
            stopping: false,
        };

        // Apply startup delay then start
        if delay > 0 {
            state.set_state(SubsystemState::Starting);
            let actor_ref = myself.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                let _ = actor_ref.cast(SubsystemMsg::Start);
            });
        } else {
            state.set_state(SubsystemState::Starting);
            spawn_process(&mut state, &myself);
        }

        Ok(state)
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SubsystemMsg::Start => {
                if state.stopping {
                    return Ok(());
                }
                state.set_state(SubsystemState::Starting);
                spawn_process(state, &myself);
            }

            SubsystemMsg::Stop { timeout_secs } => {
                state.stopping = true;
                if let Some(pid) = state.pid.take() {
                    send_signal(pid, SignalKind::Term);
                    // Schedule SIGKILL after timeout
                    let actor_ref = myself.clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(timeout_secs)).await;
                        send_signal(pid, SignalKind::Kill);
                        // If process still hasn't exited, force state
                        let _ = actor_ref.cast(SubsystemMsg::ProcessExited { exit_code: None });
                    });
                } else {
                    eprintln!("[INFO] [phoenix] Stop received for {} but process already exited", state.config.name);
                    state.set_state(SubsystemState::Stopped);
                }
            }

            SubsystemMsg::ProcessExited { exit_code } => {
                let name = &state.config.name;
                let detail = format!("subsystem={name} exit_code={exit_code:?}");
                eprintln!("[INFO] [phoenix] Process exited: {detail}");
                trauma::chronicle_record("child_exit", exit_code, None, None, Some(&detail));

                state.pid = None;

                // If we're stopping, just go to Stopped
                if state.stopping {
                    state.set_state(SubsystemState::Stopped);
                    return Ok(());
                }

                // Check restart policy
                let should_restart = match state.config.restart_policy {
                    RestartPolicy::Always => true,
                    RestartPolicy::OnFailure => exit_code != Some(0),
                    RestartPolicy::Never => false,
                };

                if !should_restart {
                    if exit_code == Some(0) {
                        state.set_state(SubsystemState::Stopped);
                    } else {
                        trauma::append_trauma(&detail);
                        state.set_state(SubsystemState::Stopped);
                    }
                    return Ok(());
                }

                if exit_code != Some(0) {
                    trauma::append_trauma(&detail);
                }

                // Check restart budget
                state.restart_count += 1;
                if state.restart_count > state.config.max_restarts {
                    let reason = format!("max restarts ({}) exceeded", state.config.max_restarts);
                    eprintln!("[ERROR] [phoenix] Subsystem {name}: {reason}");
                    trauma::chronicle_record(
                        "max_restarts",
                        exit_code,
                        Some(state.restart_count as i32),
                        Some(state.config.max_restarts as i32),
                        Some(name),
                    );
                    trauma::append_trauma(&format!("subsystem={name} {reason}"));
                    state.set_state(SubsystemState::Failed {
                        reason,
                        attempts: state.restart_count,
                    });
                    return Ok(());
                }

                // Enter backoff
                let backoff_ms = compute_backoff(
                    state.restart_count,
                    state.config.backoff_base_ms,
                    state.config.backoff_max_ms,
                );

                eprintln!(
                    "[INFO] [phoenix] Restarting {name} in {backoff_ms}ms (attempt {}/{})",
                    state.restart_count, state.config.max_restarts
                );
                trauma::chronicle_record(
                    "restart",
                    exit_code,
                    Some(state.restart_count as i32),
                    Some(state.config.max_restarts as i32),
                    Some(&format!("subsystem={name} backoff={backoff_ms}ms")),
                );

                state.set_state(SubsystemState::Backoff {
                    attempt: state.restart_count,
                    next_retry_ms: backoff_ms,
                });

                // Delayed Start
                let actor_ref = myself.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    let _ = actor_ref.cast(SubsystemMsg::Start);
                });
            }

            SubsystemMsg::GetState(reply) => {
                let _ = reply.send(state.state.clone());
            }
        }
        Ok(())
    }
}

fn spawn_process(state: &mut SubsystemActorState, myself: &ActorRef<SubsystemMsg>) {
    let cmd_str = state.config.command.clone();
    let env_vars = state.config.env.clone();
    let name = state.config.name.clone();

    let mut cmd = Command::new("sh");
    cmd.arg("-lc").arg(&cmd_str);
    for (k, v) in &env_vars {
        cmd.env(k, v);
    }

    match cmd.spawn() {
        Ok(child) => {
            let pid = child.id().unwrap_or(0);
            state.pid = Some(pid);
            state.set_state(SubsystemState::Running { pid });
            eprintln!("[INFO] [phoenix] Spawned subsystem={name} pid={pid} cmd={cmd_str}");

            // Move child into a watcher task
            let actor_ref = myself.clone();
            tokio::spawn(async move {
                let mut child = child;
                let status = child.wait().await;
                let exit_code = status.ok().and_then(|s| s.code());
                let _ = actor_ref.cast(SubsystemMsg::ProcessExited { exit_code });
            });
        }
        Err(e) => {
            eprintln!("[ERROR] [phoenix] Failed to spawn subsystem={name}: {e}");
            trauma::append_trauma(&format!("spawn-failed subsystem={name}: {e}"));
            trauma::chronicle_record(
                "spawn_failed",
                None,
                Some(state.restart_count as i32 + 1),
                Some(state.config.max_restarts as i32),
                Some(&format!("subsystem={name} error={e}")),
            );

            // Treat spawn failure like a process exit with failure
            state.restart_count += 1;
            if state.restart_count > state.config.max_restarts {
                let reason = format!("spawn failed after {} attempts: {e}", state.restart_count);
                state.set_state(SubsystemState::Failed {
                    reason,
                    attempts: state.restart_count,
                });
            } else {
                let backoff_ms = compute_backoff(
                    state.restart_count,
                    state.config.backoff_base_ms,
                    state.config.backoff_max_ms,
                );
                state.set_state(SubsystemState::Backoff {
                    attempt: state.restart_count,
                    next_retry_ms: backoff_ms,
                });
                let actor_ref = myself.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    let _ = actor_ref.cast(SubsystemMsg::Start);
                });
            }
        }
    }
}

fn compute_backoff(restart_count: u32, base_ms: u64, max_ms: u64) -> u64 {
    let shift = restart_count.min(10);
    let exp = base_ms.saturating_mul(1u64 << shift);
    let jitter = if exp > 0 {
        rand::random::<u64>() % (exp / 2).max(1)
    } else {
        0
    };
    exp.saturating_add(jitter).min(max_ms)
}

enum SignalKind {
    Term,
    Kill,
}

fn send_signal(pid: u32, kind: SignalKind) {
    #[cfg(unix)]
    {
        let sig = match kind {
            SignalKind::Term => libc::SIGTERM,
            SignalKind::Kill => libc::SIGKILL,
        };
        unsafe {
            libc::kill(pid as i32, sig);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (pid, kind);
    }
}
