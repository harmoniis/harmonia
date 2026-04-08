use crate::model::{append_tmux_metric_line, state};
use crate::tmux::cli_profiles::profile_for;
use crate::tmux::session;

use super::helpers::{get_session_and_type, get_session_name, increment_input};

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

/// Approve a permission prompt.
/// For TUI selector CLIs (Claude Code): press Enter to accept the default "Allow once".
/// For y/n CLIs (Codex): send the approve key + Enter.
pub(crate) fn approve(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);

    if profile.permission_is_selector {
        // TUI selector: "Allow once" is the default (first) option -- just Enter
        session::send_special(&sess, "Enter")?;
    } else {
        // y/n text prompt
        session::send_keys(&sess, profile.approve_key)?;
        session::send_special(&sess, "Enter")?;
    }

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

/// Deny a permission prompt.
/// For TUI selector CLIs (Claude Code): navigate Down to "Deny" option, press Enter.
/// For y/n CLIs (Codex): send the deny key + Enter.
pub(crate) fn deny(id: u64) -> Result<(), String> {
    let (sess, cli_type) = get_session_and_type(id)?;
    let profile = profile_for(&cli_type);

    if profile.permission_is_selector {
        // TUI selector: navigate to "Deny" (typically 3rd option: Allow once, Allow always, Deny)
        session::send_special(&sess, "Down")?;
        session::wait_ms(50);
        session::send_special(&sess, "Down")?;
        session::wait_ms(50);
        session::send_special(&sess, "Enter")?;
    } else {
        session::send_keys(&sess, profile.deny_key)?;
        session::send_special(&sess, "Enter")?;
    }

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
