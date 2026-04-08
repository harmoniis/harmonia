// ── Session: main TUI entry point and input loop ─────────────────────

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use console::Term;
use crossterm::{
    cursor::{SetCursorStyle, Show},
    terminal,
    ExecutableCommand,
};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use crate::bridge::wait_for_socket;
use crate::commands::dispatch_input;
use crate::input::{read_input_line, InputCallbacks};
use crate::render::{print_banner, render_buffered_response};
use crate::spinner::{init_repl_status_path, show_thinking_spinner_with_input};
use crate::theme::*;

/// Why the session ended -- determines the exit message.
pub(crate) enum ExitReason {
    UserQuit,
    CtrlC,
    ConnectionLost,
    Error(String),
}

/// Trait that the host binary implements to provide CLI-specific operations.
/// This decouples the TUI crate from the CLI binary's internal types.
pub trait SessionHost {
    // ── Paths ──
    fn socket_path(&self) -> Result<PathBuf, Box<dyn std::error::Error>>;
    fn data_dir(&self) -> Result<PathBuf, Box<dyn std::error::Error>>;
    fn node_label(&self) -> &str;
    fn session_id(&self) -> &str;

    // ── Startup ──
    fn ensure_daemon(&self) -> Result<(), Box<dyn std::error::Error>>;
    fn append_session_event(&self, actor: &str, kind: &str, text: &str);

    // ── Input callbacks factory ──
    fn create_input_callbacks(&self) -> Box<dyn InputCallbacks>;

    // ── Commands ──
    fn print_help(&self);
    fn print_session_summary(&self);
    fn print_status(&self);
    fn print_providers(&self);
    fn print_recent_log(&self);
    fn clear_and_new_session(&self, term: &Term);
    fn run_rewind_flow(&self, stdout: &mut std::io::Stdout, term: &Term);
    fn run_resume_flow(&self, stdout: &mut std::io::Stdout);
    fn run_policies_flow(&self, stdout: &mut std::io::Stdout);
    fn run_frontends(&self, stdout: &mut std::io::Stdout);
    #[cfg(unix)]
    fn run_menu_flow(
        &self,
        stdout: &mut std::io::Stdout,
        writer: &mut UnixStream,
        waiting: &Arc<AtomicBool>,
        running: &Arc<AtomicBool>,
        reader_alive: &Arc<AtomicBool>,
        response_buf: &Arc<Mutex<Vec<String>>>,
        assistant_label: &str,
    );
}

#[cfg(unix)]
pub fn run(host: &dyn SessionHost) -> Result<(), Box<dyn std::error::Error>> {
    let term = Term::stderr();
    let socket_path = host.socket_path()?;

    if !socket_path.exists() {
        host.ensure_daemon()?;
        wait_for_socket(
            &socket_path,
            "Waiting for daemon...",
            &format!(
                "daemon started but socket not ready — check logs\n\
                 \x20 expected socket: {}",
                socket_path.display(),
            ),
        )?;
    }

    // Initialize repl-status path
    if let Ok(data_dir) = host.data_dir() {
        init_repl_status_path(&data_dir.to_string_lossy());
    }

    let stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("cannot connect to session service — is it running? ({})", e))?;

    let reader_stream = stream.try_clone()?;
    reader_stream.set_read_timeout(Some(std::time::Duration::from_millis(300)))?;
    let mut writer_stream = stream;

    // Print banner
    print_banner(&term, host.node_label(), host.session_id());
    host.append_session_event(
        "system",
        "session-open",
        &format!(
            "node={} socket={}",
            host.node_label(),
            socket_path.display()
        ),
    );

    // Shared state
    let waiting = Arc::new(AtomicBool::new(false));
    let waiting_reader = Arc::clone(&waiting);
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);
    let reader_alive = Arc::new(AtomicBool::new(true));
    let reader_alive_writer = Arc::clone(&reader_alive);
    let response_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let response_buf_reader = Arc::clone(&response_buf);

    // Ctrl+C
    let _ = ctrlc::set_handler(move || {
        running_ctrlc.store(false, Ordering::Relaxed);
        let _ = terminal::disable_raw_mode();
        let _ = std::io::stderr().execute(Show);
        eprintln!();
    });

    // Response reader thread
    let running_reader = Arc::clone(&running);
    let reader_handle = std::thread::spawn(move || {
        let mut reader = BufReader::new(reader_stream);
        let mut in_response = false;
        let mut line_buf = String::new();

        loop {
            if !running_reader.load(Ordering::Relaxed) {
                break;
            }

            line_buf.clear();
            match reader.read_line(&mut line_buf) {
                Ok(0) => break,
                Ok(_) => {
                    let line = line_buf.trim_end_matches('\n').trim_end_matches('\r');
                    in_response = true;
                    if let Ok(mut buf) = response_buf_reader.lock() {
                        buf.push(line.to_string());
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if in_response {
                        in_response = false;
                        waiting_reader.store(false, Ordering::Release);
                    }
                    continue;
                }
                Err(_) => break,
            }
        }

        waiting_reader.store(false, Ordering::Release);
        reader_alive_writer.store(false, Ordering::Release);
    });

    let assistant_label = format!("harmonia@{}", host.node_label());

    // Main input loop
    let exit_reason = run_input_loop(
        &mut writer_stream,
        &waiting,
        &running,
        &reader_alive,
        &response_buf,
        &assistant_label,
        &term,
        host,
    );

    // Drain any final buffered response
    render_buffered_response(&response_buf, &assistant_label);

    running.store(false, Ordering::Relaxed);
    let _ = writer_stream.shutdown(std::net::Shutdown::Both);
    let _ = reader_handle.join();

    // Restore terminal
    let _ = terminal::disable_raw_mode();
    let _ = std::io::stderr().execute(SetCursorStyle::DefaultUserShape);
    let _ = std::io::stderr().execute(Show);

    match &exit_reason {
        ExitReason::UserQuit => {
            eprintln!();
            eprintln!("  {BOLD_CYAN}◆{RESET} Goodbye.");
            eprintln!();
        }
        ExitReason::CtrlC => {
            eprintln!();
            eprintln!("  {BOLD_CYAN}◆{RESET} Goodbye.");
            eprintln!();
        }
        ExitReason::ConnectionLost => {
            eprintln!();
            eprintln!("  {RED}✗{RESET} Connection lost — daemon may have stopped.");
            eprintln!("  Run {CYAN}harmonia status{RESET} to check.");
            eprintln!();
        }
        ExitReason::Error(e) => {
            eprintln!();
            eprintln!("  {RED}✗{RESET} Session error: {e}");
            eprintln!();
        }
    }

    match exit_reason {
        ExitReason::Error(e) => Err(e.into()),
        _ => Ok(()),
    }
}

#[cfg(unix)]
fn run_input_loop(
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
    term: &Term,
    host: &dyn SessionHost,
) -> ExitReason {
    let mut stdout = std::io::stdout();
    let mut queued_input: Option<String> = None;
    let mut input_cb = host.create_input_callbacks();
    let mut first_input = true;

    loop {
        if !running.load(Ordering::Relaxed) {
            return if !reader_alive.load(Ordering::Relaxed) {
                ExitReason::ConnectionLost
            } else {
                ExitReason::CtrlC
            };
        }

        // If waiting for a response, show spinner
        if waiting.load(Ordering::Acquire) {
            let input = show_thinking_spinner_with_input(waiting, running, reader_alive);
            render_buffered_response(response_buf, assistant_label);
            if !running.load(Ordering::Relaxed) {
                continue;
            }
            if let Some(text) = input {
                let trimmed = text.trim().to_string();
                if !trimmed.is_empty() {
                    queued_input = Some(trimmed);
                }
            }
        }

        render_buffered_response(response_buf, assistant_label);

        // Drain queued input
        if let Some(pending) = queued_input.take() {
            if let Err(e) = dispatch_input(
                &pending,
                writer,
                waiting,
                running,
                reader_alive,
                response_buf,
                assistant_label,
                term,
                &mut stdout,
                host,
            ) {
                return ExitReason::Error(e.to_string());
            }
            continue;
        }

        // Read input
        let restore_draft = first_input;
        first_input = false;
        let input = match read_input_line(running, input_cb.as_mut(), restore_draft) {
            Ok(s) => s,
            Err(e) => return ExitReason::Error(e.to_string()),
        };

        if !running.load(Ordering::Relaxed) {
            continue;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Handle /exit here for clean ExitReason
        if trimmed == "/exit" || trimmed == "/quit" || trimmed == "/q" {
            host.append_session_event("you", "user", trimmed);
            return ExitReason::UserQuit;
        }

        if let Err(e) = dispatch_input(
            trimmed,
            writer,
            waiting,
            running,
            reader_alive,
            response_buf,
            assistant_label,
            term,
            &mut stdout,
            host,
        ) {
            return ExitReason::Error(e.to_string());
        }
    }
}
