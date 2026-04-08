use crate::model::{state, CliState, CliType, append_tmux_metric_line};

pub(crate) fn get_session_name(id: u64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let agent = st
        .tmux_agents
        .get(&id)
        .ok_or_else(|| format!("tmux agent {id} not found"))?;
    Ok(agent.session_name.clone())
}

pub(crate) fn get_session_and_type(id: u64) -> Result<(String, CliType), String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let agent = st
        .tmux_agents
        .get(&id)
        .ok_or_else(|| format!("tmux agent {id} not found"))?;
    Ok((agent.session_name.clone(), agent.cli_type.clone()))
}

pub(crate) fn increment_input(id: u64, event: &str) {
    if let Ok(mut st) = state().write() {
        if let Some(agent) = st.tmux_agents.get_mut(&id) {
            agent.total_inputs_sent += 1;
            append_tmux_metric_line(agent, event);
        }
    }
}

pub(crate) fn state_label(state: &CliState) -> &'static str {
    match state {
        CliState::Launching => "launching",
        CliState::WaitingForInput => "waiting-input",
        CliState::Processing => "processing",
        CliState::WaitingForPermission { .. } => "waiting-permission",
        CliState::WaitingForConfirmation { .. } => "waiting-confirmation",
        CliState::WaitingForSelection { .. } => "waiting-selection",
        CliState::Onboarding => "onboarding",
        CliState::PlanMode => "plan-mode",
        CliState::Completed => "completed",
        CliState::Error(_) => "error",
        CliState::Terminated => "terminated",
    }
}
