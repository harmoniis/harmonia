use crate::model::{state, CliType};
use crate::tmux::{controller, detector, session};

pub fn tmux_spawn(cli_type_str: &str, workdir: &str, prompt: &str) -> Result<i64, String> {
    let cli_type = CliType::from_str(cli_type_str)?;
    controller::spawn(&cli_type, workdir, prompt).map(|id| id as i64)
}

pub fn tmux_spawn_custom(
    command: &str,
    args: &str,
    workdir: &str,
    prompt: &str,
) -> Result<i64, String> {
    let shell_args: Vec<String> = if args.is_empty() {
        vec![]
    } else {
        args.split_whitespace().map(|s| s.to_string()).collect()
    };
    let cli_type = CliType::Custom {
        command: command.to_string(),
        shell_args,
    };
    controller::spawn(&cli_type, workdir, prompt).map(|id| id as i64)
}

pub fn tmux_poll(id: i64) -> Result<String, String> {
    let state = controller::poll(id as u64)?;
    Ok(state.to_sexp())
}

pub fn tmux_send(id: i64, input: &str) -> Result<(), String> {
    controller::send_input(id as u64, input)
}

pub fn tmux_send_key(id: i64, key: &str) -> Result<(), String> {
    controller::send_key(id as u64, key)
}

pub fn tmux_approve(id: i64) -> Result<(), String> {
    controller::approve(id as u64)
}

pub fn tmux_deny(id: i64) -> Result<(), String> {
    controller::deny(id as u64)
}

pub fn tmux_confirm_yes(id: i64) -> Result<(), String> {
    controller::confirm_yes(id as u64)
}

pub fn tmux_confirm_no(id: i64) -> Result<(), String> {
    controller::confirm_no(id as u64)
}

pub fn tmux_select(id: i64, index: i32) -> Result<(), String> {
    controller::select_option(id as u64, index as usize)
}

pub fn tmux_capture(id: i64, history: i32) -> Result<String, String> {
    let h = if history <= 0 { 200 } else { history as u32 };
    controller::capture(id as u64, h)
}

pub fn tmux_kill(id: i64) -> Result<(), String> {
    controller::kill(id as u64)
}

pub fn tmux_interrupt(id: i64) -> Result<(), String> {
    controller::interrupt(id as u64)
}

pub fn tmux_status(id: i64) -> Result<String, String> {
    controller::agent_status(id as u64)
}

pub fn tmux_list() -> Result<String, String> {
    controller::list()
}

pub fn tmux_swarm_poll() -> Result<String, String> {
    controller::swarm_poll()
}

/// Capture raw output and extract the meaningful response (strip TUI chrome).
pub fn tmux_extract_response(id: i64) -> Result<String, String> {
    let cli_type = {
        let st = state()
            .read()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        let agent = st
            .tmux_agents
            .get(&(id as u64))
            .ok_or_else(|| format!("tmux agent {} not found", id))?;
        agent.cli_type.clone()
    };
    let raw = controller::capture(id as u64, 200)?;
    Ok(detector::extract_response(&raw, &cli_type))
}

/// Capture only the visible pane (no scrollback).
pub fn tmux_capture_visible(id: i64) -> Result<String, String> {
    let sess = {
        let st = state()
            .read()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        let agent = st
            .tmux_agents
            .get(&(id as u64))
            .ok_or_else(|| format!("tmux agent {} not found", id))?;
        agent.session_name.clone()
    };
    session::capture_visible(&sess)
}

/// List tmux sessions managed by harmonia.
pub fn tmux_sessions() -> Result<Vec<String>, String> {
    session::list_sessions_with_prefix("harmonia-")
}
