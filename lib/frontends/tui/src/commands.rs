// ── Commands: slash command dispatch ──────────────────────────────────

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use console::Term;

use crate::bridge::send_to_daemon;
use crate::render::print_wrapped;
use crate::theme::*;

pub(crate) enum CommandResult {
    Handled,
    Quit,
    SendToAgent(String),
    SessionText,
}

#[cfg(unix)]
pub(crate) fn handle_command(
    cmd: &str,
    term: &Term,
    stdout: &mut std::io::Stdout,
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
    host: &dyn crate::session::SessionHost,
) -> CommandResult {
    let base = cmd.split_whitespace().next().unwrap_or("");
    match base {
        "/help" | "/h" | "/?" => {
            host.print_help();
            CommandResult::Handled
        }
        "/session" => {
            host.print_session_summary();
            CommandResult::Handled
        }
        "/clear" | "/cls" => {
            host.clear_and_new_session(term);
            CommandResult::Handled
        }
        "/log" | "/logs" => {
            host.print_recent_log();
            CommandResult::Handled
        }
        "/quit" | "/exit" | "/q" => CommandResult::Quit,

        "/rewind" => {
            host.run_rewind_flow(stdout, term);
            CommandResult::Handled
        }

        "/resume" => {
            host.run_resume_flow(stdout);
            CommandResult::Handled
        }

        "/policies" => {
            host.run_policies_flow(stdout);
            CommandResult::Handled
        }

        "/menu" | "/m" => {
            host.run_menu_flow(
                stdout,
                writer,
                waiting,
                running,
                reader_alive,
                response_buf,
                assistant_label,
            );
            CommandResult::Handled
        }

        "/pair" | "/link" => {
            eprintln!(
                "\n  {DIM}Use Frontends from /menu. /pair is a compatibility alias.{RESET}\n"
            );
            host.run_frontends(stdout);
            CommandResult::Handled
        }

        "/frontends" => {
            host.run_frontends(stdout);
            CommandResult::Handled
        }

        "/status" => {
            host.print_status();
            CommandResult::Handled
        }

        "/providers" | "/backends" => {
            host.print_providers();
            CommandResult::Handled
        }

        "/tools" | "/chronicle" | "/metrics" | "/security" | "/identity" | "/feedback"
        | "/wallet" => CommandResult::SendToAgent(cmd.to_string()),

        _ => CommandResult::SessionText,
    }
}

/// Dispatch a user input string: handle commands, echo, send to daemon.
#[cfg(unix)]
pub(crate) fn dispatch_input(
    trimmed: &str,
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
    term: &Term,
    stdout: &mut std::io::Stdout,
    host: &dyn crate::session::SessionHost,
) -> Result<(), Box<dyn std::error::Error>> {
    host.append_session_event("you", "user", trimmed);

    if trimmed.starts_with('/') {
        match handle_command(
            trimmed,
            term,
            stdout,
            writer,
            waiting,
            running,
            reader_alive,
            response_buf,
            assistant_label,
            host,
        ) {
            CommandResult::Handled => return Ok(()),
            CommandResult::Quit => {
                running.store(false, Ordering::Relaxed);
                return Ok(());
            }
            CommandResult::SendToAgent(cmd) => {
                send_to_daemon(writer, &cmd, waiting, running)?;
                return Ok(());
            }
            CommandResult::SessionText => {}
        }
    }

    // Print user message echo
    let node_label = host.node_label();
    eprintln!();
    eprintln!();
    eprintln!("  {BOLD_GREEN}╭─{RESET} {DIM}you@{node_label}{RESET}");
    let user_prefix = format!("  {GREEN}│{RESET} ");
    for line in trimmed.lines() {
        print_wrapped(line, &user_prefix, &user_prefix, "");
    }
    eprintln!("  {BOLD_GREEN}╰─{RESET}");
    eprintln!();

    send_to_daemon(writer, trimmed, waiting, running)?;
    Ok(())
}
