//! Tmux RPC operations (list, spawn, capture, send).

use harmonia_node_rpc::{NodePathRef, NodeRpcResult};
use std::path::Path;
use std::process::Command;

use super::helpers::{default_exec_cwd, resolve_path};

pub(crate) fn tmux_list() -> Result<NodeRpcResult, String> {
    Ok(NodeRpcResult::TmuxList {
        sessions: tmux_list_sessions()?,
    })
}

pub(crate) fn tmux_spawn(
    node: &crate::paths::NodeIdentity,
    session_name: &str,
    cwd: Option<&NodePathRef>,
    command: Option<&str>,
    args: &[String],
) -> Result<NodeRpcResult, String> {
    let cwd = match cwd {
        Some(path_ref) => resolve_path(node, path_ref)?,
        None => default_exec_cwd(node)?,
    };
    tmux_spawn_session(session_name, &cwd, command, args)?;
    Ok(NodeRpcResult::TmuxSpawn {
        session_name: session_name.to_string(),
    })
}

pub(crate) fn tmux_capture(
    session_name: &str,
    history_lines: u32,
) -> Result<NodeRpcResult, String> {
    Ok(NodeRpcResult::TmuxCapture {
        session_name: session_name.to_string(),
        output: tmux_capture_pane(session_name, history_lines)?,
    })
}

pub(crate) fn tmux_send_line(session_name: &str, input: &str) -> Result<NodeRpcResult, String> {
    tmux_send_line_raw(session_name, input)?;
    Ok(NodeRpcResult::TmuxSendLine {
        session_name: session_name.to_string(),
    })
}

pub(crate) fn tmux_send_key(session_name: &str, key: &str) -> Result<NodeRpcResult, String> {
    tmux_send_key_raw(session_name, key)?;
    Ok(NodeRpcResult::TmuxSendKey {
        session_name: session_name.to_string(),
        key: key.to_string(),
    })
}

// --- Internal helpers ---

fn tmux_run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .map_err(|e| format!("tmux exec failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            return Err("tmux command failed".to_string());
        }
        return Err(format!("tmux error: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn tmux_list_sessions() -> Result<Vec<String>, String> {
    match tmux_run(&["list-sessions", "-F", "#{session_name}"]) {
        Ok(output) => Ok(output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect()),
        Err(err) if err.contains("no server running") || err.contains("no sessions") => Ok(vec![]),
        Err(err) => Err(err),
    }
}

fn tmux_spawn_session(
    session_name: &str,
    cwd: &Path,
    command: Option<&str>,
    args: &[String],
) -> Result<(), String> {
    let mut cmd = Command::new("tmux");
    cmd.arg("new-session")
        .arg("-d")
        .arg("-s")
        .arg(session_name)
        .arg("-c")
        .arg(cwd);
    if let Some(command) = command {
        cmd.arg(command);
        for arg in args {
            cmd.arg(arg);
        }
    }
    let output = cmd
        .output()
        .map_err(|e| format!("tmux spawn failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "tmux spawn error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn tmux_capture_pane(session_name: &str, history_lines: u32) -> Result<String, String> {
    tmux_run(&[
        "capture-pane",
        "-t",
        session_name,
        "-p",
        "-S",
        &format!("-{}", history_lines.max(1)),
    ])
}

fn tmux_send_line_raw(session_name: &str, input: &str) -> Result<(), String> {
    tmux_run(&["send-keys", "-t", session_name, "-l", input])?;
    tmux_run(&["send-keys", "-t", session_name, "Enter"])?;
    Ok(())
}

fn tmux_send_key_raw(session_name: &str, key: &str) -> Result<(), String> {
    tmux_run(&["send-keys", "-t", session_name, key])?;
    Ok(())
}
