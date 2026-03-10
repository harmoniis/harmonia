//! Low-level tmux command wrappers.
//!
//! Every function here is a thin shell over `tmux` subcommands.
//! No state, no detection logic — pure I/O.

use std::process::Command;

const TMUX: &str = "tmux";

/// Environment variables that must be cleared in tmux sessions to allow
/// CLI agents to spawn without nesting-protection errors.
/// Discovered via live testing: Claude Code sets CLAUDECODE=1 and
/// CLAUDE_CODE_ENTRYPOINT in parent shells, then refuses to nest.
const SANITIZE_ENV_VARS: &[&str] = &["CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT"];

fn run(args: &[&str]) -> Result<String, String> {
    let out = Command::new(TMUX)
        .args(args)
        .output()
        .map_err(|e| format!("tmux exec failed: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err(format!("tmux error: {stderr}"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Create a new detached tmux session with the given name, working directory,
/// and initial window size suitable for CLI agent capture.
/// After creation, sanitizes the environment by unsetting variables that
/// would prevent CLI agents from launching (e.g. CLAUDECODE nesting guard).
pub(crate) fn create_session(name: &str, workdir: &str) -> Result<(), String> {
    run(&[
        "new-session",
        "-d",
        "-s",
        name,
        "-c",
        workdir,
        "-x",
        "220",
        "-y",
        "50",
    ])?;

    // Sanitize the environment: unset vars that block CLI nesting
    let unset_cmd = SANITIZE_ENV_VARS
        .iter()
        .map(|v| format!("unset {v}"))
        .collect::<Vec<_>>()
        .join(" && ");
    run(&["send-keys", "-t", name, "-l", &unset_cmd])?;
    run(&["send-keys", "-t", name, "Enter"])?;

    Ok(())
}

/// Check whether a tmux session with the given name exists.
pub(crate) fn session_exists(name: &str) -> bool {
    run(&["has-session", "-t", name]).is_ok()
}

/// Kill (destroy) a tmux session.
pub(crate) fn kill_session(name: &str) -> Result<(), String> {
    run(&["kill-session", "-t", name])?;
    Ok(())
}

/// Send a sequence of literal keys to the session pane.
/// This is the raw `send-keys` — caller decides whether to append Enter.
pub(crate) fn send_keys(name: &str, keys: &str) -> Result<(), String> {
    run(&["send-keys", "-t", name, keys])?;
    Ok(())
}

/// Send text followed by Enter — the most common input pattern.
pub(crate) fn send_line(name: &str, text: &str) -> Result<(), String> {
    // Use -l (literal) to prevent tmux from interpreting special chars,
    // then send Enter separately.
    run(&["send-keys", "-t", name, "-l", text])?;
    run(&["send-keys", "-t", name, "Enter"])?;
    Ok(())
}

/// Send a special key (Enter, Tab, Escape, Up, Down, etc.).
pub(crate) fn send_special(name: &str, key: &str) -> Result<(), String> {
    run(&["send-keys", "-t", name, key])?;
    Ok(())
}

/// Capture the visible pane content plus `history` lines of scrollback.
pub(crate) fn capture_pane(name: &str, history_lines: u32) -> Result<String, String> {
    let start = format!("-{}", history_lines);
    run(&["capture-pane", "-t", name, "-p", "-S", &start])
}

/// Capture only the visible pane (no scrollback).
#[allow(dead_code)]
pub(crate) fn capture_visible(name: &str) -> Result<String, String> {
    run(&["capture-pane", "-t", name, "-p"])
}

/// List all tmux sessions whose names start with a given prefix.
#[allow(dead_code)]
pub(crate) fn list_sessions_with_prefix(prefix: &str) -> Result<Vec<String>, String> {
    let output = match run(&["list-sessions", "-F", "#{session_name}"]) {
        Ok(v) => v,
        Err(e) => {
            // "no server running" means zero sessions — not an error for us
            if e.contains("no server running") || e.contains("no sessions") {
                return Ok(vec![]);
            }
            return Err(e);
        }
    };
    Ok(output
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(|l| l.to_string())
        .collect())
}

/// Send Ctrl+C (interrupt) to a session.
pub(crate) fn send_interrupt(name: &str) -> Result<(), String> {
    run(&["send-keys", "-t", name, "C-c"])?;
    Ok(())
}

/// Wait for a given duration (milliseconds) — utility for polling loops.
pub(crate) fn wait_ms(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}
