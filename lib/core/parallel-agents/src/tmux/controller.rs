//! High-level tmux agent controller.
//!
//! The controller is the conductor's hand — it manages the full lifecycle
//! of tmux CLI agents: spawn, monitor, input, terminate. It connects the
//! low-level session commands with the detection engine and the global state.
//!
//! Architecture:
//! - Each CLI agent runs in its own tmux session named `harmonia-{id}`
//! - The controller polls sessions to detect state changes
//! - Input decisions come from the Lisp orchestrator via FFI
//! - Metrics are logged for every interaction (harmonic telemetry)

use super::cli_profiles::profile_for;
use super::detector::detect_state;
use super::session;
use crate::actor_core::{self, ActorKind, MessagePayload};
use crate::model::{append_tmux_metric_line, now_unix, state, CliState, CliType, TmuxAgent};

const SESSION_PREFIX: &str = "harmonia-";

fn session_name(id: u64) -> String {
    format!("{SESSION_PREFIX}{id}")
}

/// Spawn a new tmux CLI agent.
///
/// Creates a tmux session, launches the CLI tool, and optionally sends
/// an initial prompt. Returns the agent ID.
pub(crate) fn spawn(
    cli_type: &CliType,
    workdir: &str,
    initial_prompt: &str,
) -> Result<u64, String> {
    let id = {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        let id = st.next_id;
        st.next_id += 1;
        id
    };

    let sess = session_name(id);

    // Create the tmux session in the target working directory.
    // create_session automatically sanitizes the environment (unsets CLAUDECODE etc.)
    session::create_session(&sess, workdir)?;

    // Small delay to let the shell initialize and env sanitization to complete
    session::wait_ms(500);

    // Launch strategy depends on whether we have an initial prompt.
    // If prompt is given, use non-interactive mode (claude -p / codex exec)
    // which runs the task and exits — minimal token usage, no session overhead.
    // If no prompt, launch interactive mode so the conductor can send prompts later.
    if !initial_prompt.is_empty() {
        let (cmd, args) = cli_type.launch_command_noninteractive(initial_prompt);
        let full_cmd = if args.is_empty() {
            cmd
        } else {
            // Shell-escape single quotes in args for safe tmux send
            let escaped_args: Vec<String> = args
                .iter()
                .map(|a| {
                    if a.contains(' ') || a.contains('\'') || a.contains('"') {
                        format!("'{}'", a.replace('\'', "'\\''"))
                    } else {
                        a.clone()
                    }
                })
                .collect();
            format!("{} {}", cmd, escaped_args.join(" "))
        };
        session::send_line(&sess, &full_cmd)?;
    } else {
        let (cmd, args) = cli_type.launch_command();
        let full_cmd = if args.is_empty() {
            cmd.to_string()
        } else {
            format!("{} {}", cmd, args.join(" "))
        };
        session::send_line(&sess, &full_cmd)?;
    }

    // Wait for CLI to boot
    session::wait_ms(1500);

    let agent = TmuxAgent {
        id,
        cli_type: cli_type.clone(),
        session_name: sess,
        workdir: workdir.to_string(),
        initial_prompt: initial_prompt.to_string(),
        state: CliState::Launching,
        created_at: now_unix(),
        last_output: String::new(),
        last_poll_at: 0,
        interaction_count: 0,
        total_inputs_sent: 0,
        permissions_approved: 0,
        permissions_denied: 0,
        estimated_cost_usd: 0.0,
        duration_ms: 0,
    };

    append_tmux_metric_line(&agent, "spawn");

    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        st.tmux_agents.insert(id, agent);
    }

    // When launched with initial_prompt, the CLI is already running the task
    // via non-interactive mode (claude -p / codex exec). No need to send
    // the prompt again — it was baked into the launch command.
    // When launched without a prompt (interactive mode), the conductor
    // will poll and send prompts via send_input() when the CLI is ready.

    Ok(id)
}

/// Poll the current state of a tmux agent by capturing its terminal output
/// and running detection.
pub(crate) fn poll(id: u64) -> Result<CliState, String> {
    let (sess, cli_type) = {
        let st = state()
            .read()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        let agent = st
            .tmux_agents
            .get(&id)
            .ok_or_else(|| format!("tmux agent {id} not found"))?;
        (agent.session_name.clone(), agent.cli_type.clone())
    };

    // Check if session still exists
    if !session::session_exists(&sess) {
        {
            let mut st = state()
                .write()
                .map_err(|_| "parallel state lock poisoned".to_string())?;
            if let Some(agent) = st.tmux_agents.get_mut(&id) {
                agent.state = CliState::Terminated;
                agent.last_poll_at = now_unix();
                append_tmux_metric_line(agent, "terminated");
            }
        }
        // Push failed message to unified mailbox
        if let Ok(mut reg) = actor_core::registry().write() {
            reg.post_from(
                id,
                0,
                ActorKind::CliAgent,
                MessagePayload::TaskFailed {
                    error: "tmux session terminated unexpectedly".to_string(),
                    duration_ms: 0,
                },
            );
        }
        return Ok(CliState::Terminated);
    }

    let output = session::capture_pane(&sess, 200)?;
    let detected = detect_state(&output, &cli_type);

    {
        // Collect messages to post to unified mailbox after releasing state lock
        let mut pending_payloads: Vec<MessagePayload> = Vec::new();

        {
            let mut st = state()
                .write()
                .map_err(|_| "parallel state lock poisoned".to_string())?;

            if let Some(agent) = st.tmux_agents.get_mut(&id) {
                let prev_state = agent.state.clone();
                let prev_output_len = agent.last_output.len();
                agent.state = detected.clone();
                agent.last_output = output.clone();
                agent.last_poll_at = now_unix();
                agent.interaction_count += 1;

                // Accumulate cost and duration tracking
                agent.duration_ms = (now_unix() - agent.created_at) * 1000;
                if matches!(detected, CliState::Processing) {
                    agent.estimated_cost_usd += agent.cli_type.estimated_cost_per_interaction();
                }

                let dur = agent.duration_ms;

                // Log state transitions and collect mailbox messages
                if std::mem::discriminant(&prev_state) != std::mem::discriminant(&detected) {
                    append_tmux_metric_line(agent, &format!("state:{}", state_label(&detected)));

                    pending_payloads.push(MessagePayload::StateChanged {
                        to: detected.to_sexp(),
                    });

                    match &detected {
                        CliState::Completed => {
                            pending_payloads.push(MessagePayload::TaskCompleted {
                                output: output.clone(),
                                exit_code: 0,
                                duration_ms: dur,
                            });
                        }
                        CliState::Error(e) => {
                            pending_payloads.push(MessagePayload::TaskFailed {
                                error: e.clone(),
                                duration_ms: dur,
                            });
                        }
                        _ => {}
                    }
                }

                // Progress heartbeat if output changed
                let bytes_delta = if output.len() > prev_output_len {
                    (output.len() - prev_output_len) as u64
                } else {
                    0
                };
                if bytes_delta > 0 {
                    pending_payloads.push(MessagePayload::ProgressHeartbeat { bytes_delta });
                }
            }
        }

        // Post all collected messages to unified mailbox
        if !pending_payloads.is_empty() {
            if let Ok(mut reg) = actor_core::registry().write() {
                for payload in pending_payloads {
                    reg.post_from(id, 0, ActorKind::CliAgent, payload);
                }
            }
        }
    }

    Ok(detected)
}

/// Send free-text input to a tmux agent (types text + Enter).
pub(crate) fn send_input(id: u64, input: &str) -> Result<(), String> {
    let sess = get_session_name(id)?;
    session::send_line(&sess, input)?;

    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.total_inputs_sent += 1;
            append_tmux_metric_line(agent, "input");
        }
    }
    Ok(())
}

/// Send a special key to a tmux agent (Enter, Tab, Escape, Up, Down, etc.).
pub(crate) fn send_key(id: u64, key: &str) -> Result<(), String> {
    let sess = get_session_name(id)?;
    session::send_special(&sess, key)?;
    Ok(())
}

/// Approve a permission prompt — sends the profile's approve key.
pub(crate) fn approve(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);
    session::send_keys(&sess, profile.approve_key)?;
    session::send_special(&sess, "Enter")?;

    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.permissions_approved += 1;
            agent.total_inputs_sent += 1;
            append_tmux_metric_line(agent, "approve");
        }
    }
    Ok(())
}

/// Deny a permission prompt — sends the profile's deny key.
pub(crate) fn deny(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);
    session::send_keys(&sess, profile.deny_key)?;
    session::send_special(&sess, "Enter")?;

    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.permissions_denied += 1;
            agent.total_inputs_sent += 1;
            append_tmux_metric_line(agent, "deny");
        }
    }
    Ok(())
}

/// Confirm (yes) a confirmation prompt.
pub(crate) fn confirm_yes(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);
    session::send_keys(&sess, profile.yes_key)?;
    session::send_special(&sess, "Enter")?;
    increment_input(id, "confirm-yes");
    Ok(())
}

/// Deny a confirmation prompt.
pub(crate) fn confirm_no(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);
    session::send_keys(&sess, profile.no_key)?;
    session::send_special(&sess, "Enter")?;
    increment_input(id, "confirm-no");
    Ok(())
}

/// Select an option by index (0-based) using arrow keys + Enter.
pub(crate) fn select_option(id: u64, index: usize) -> Result<(), String> {
    let sess = get_session_name(id)?;
    // Move down to the desired option
    for _ in 0..index {
        session::send_special(&sess, "Down")?;
        session::wait_ms(50);
    }
    session::send_special(&sess, "Enter")?;
    increment_input(id, "select");
    Ok(())
}

/// Interrupt the CLI agent (Ctrl+C).
pub(crate) fn interrupt(id: u64) -> Result<(), String> {
    let sess = get_session_name(id)?;
    session::send_interrupt(&sess)?;
    Ok(())
}

/// Capture the current terminal output of a tmux agent.
pub(crate) fn capture(id: u64, history: u32) -> Result<String, String> {
    let sess = get_session_name(id)?;
    session::capture_pane(&sess, history)
}

/// Kill a tmux agent, destroying its session.
pub(crate) fn kill(id: u64) -> Result<(), String> {
    let sess = get_session_name(id)?;
    if session::session_exists(&sess) {
        session::kill_session(&sess)?;
    }

    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.state = CliState::Terminated;
            append_tmux_metric_line(agent, "kill");
        }
    }
    Ok(())
}

/// List all active tmux agents as an s-expression.
pub(crate) fn list() -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;

    let mut entries: Vec<String> = st.tmux_agents.values().map(|a| a.to_sexp()).collect();
    entries.sort();
    Ok(format!("({})", entries.join(" ")))
}

/// Get a full status report of a specific tmux agent as s-expression.
pub(crate) fn agent_status(id: u64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let agent = st
        .tmux_agents
        .get(&id)
        .ok_or_else(|| format!("tmux agent {id} not found"))?;
    Ok(agent.to_sexp())
}

/// Poll ALL active tmux agents and return their collective state as s-expression.
/// This is the swarm heartbeat — called periodically by the conductor.
pub(crate) fn swarm_poll() -> Result<String, String> {
    let agent_ids: Vec<u64> = {
        let st = state()
            .read()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        st.tmux_agents.keys().copied().collect()
    };

    let mut results = Vec::new();
    let mut needs_input = 0u32;
    let mut processing = 0u32;
    let mut completed = 0u32;
    let mut errors = 0u32;
    let mut total = 0u32;

    for id in agent_ids {
        match poll(id) {
            Ok(state) => {
                total += 1;
                match &state {
                    CliState::WaitingForInput
                    | CliState::WaitingForPermission { .. }
                    | CliState::WaitingForConfirmation { .. }
                    | CliState::WaitingForSelection { .. } => needs_input += 1,
                    CliState::Processing | CliState::Launching => processing += 1,
                    CliState::Completed | CliState::Terminated => completed += 1,
                    CliState::Error(_) => errors += 1,
                }
                results.push(format!("(:id {} :state {})", id, state.to_sexp()));
            }
            Err(e) => {
                errors += 1;
                results.push(format!(
                    "(:id {} :state (:error \"{}\"))",
                    id,
                    crate::model::json_escape(&e)
                ));
            }
        }
    }

    Ok(format!(
        "(:swarm-status :total {} :needs-input {} :processing {} :completed {} :errors {} :agents ({}))",
        total, needs_input, processing, completed, errors,
        results.join(" ")
    ))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn get_session_name(id: u64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let agent = st
        .tmux_agents
        .get(&id)
        .ok_or_else(|| format!("tmux agent {id} not found"))?;
    Ok(agent.session_name.clone())
}

fn get_session_and_type(id: u64) -> Result<(String, CliType), String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let agent = st
        .tmux_agents
        .get(&id)
        .ok_or_else(|| format!("tmux agent {id} not found"))?;
    Ok((agent.session_name.clone(), agent.cli_type.clone()))
}

fn increment_input(id: u64, event: &str) {
    if let Ok(mut st) = state().write() {
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.total_inputs_sent += 1;
            append_tmux_metric_line(agent, event);
        }
    }
}

fn state_label(state: &CliState) -> &'static str {
    match state {
        CliState::Launching => "launching",
        CliState::WaitingForInput => "waiting-input",
        CliState::Processing => "processing",
        CliState::WaitingForPermission { .. } => "waiting-permission",
        CliState::WaitingForConfirmation { .. } => "waiting-confirmation",
        CliState::WaitingForSelection { .. } => "waiting-selection",
        CliState::Completed => "completed",
        CliState::Error(_) => "error",
        CliState::Terminated => "terminated",
    }
}
