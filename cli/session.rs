use crate::start;
use console::{style, Term};
use crossterm::{
    cursor::{self, Hide, MoveTo, RestorePosition, SavePosition, SetCursorStyle, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use unicode_width::UnicodeWidthChar;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_INPUT_LINES: usize = 10;

const LOGO: &str = r#"
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
"#;

// ── Colors ────────────────────────────────────────────────────────────
// Harmonia gradient: violet → dark blue → cyan → cyan-green

const CYAN: &str = "\x1b[36m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[32m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD_WHITE: &str = "\x1b[1;37m";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let term = Term::stderr();
    let node_identity = crate::paths::current_node_identity()?;
    let session = Arc::new(crate::paths::create_session(&node_identity)?);
    let socket_path = crate::paths::socket_path()?;

    if !socket_path.exists() {
        if node_identity.install_profile == crate::paths::InstallProfile::TuiClient {
            eprintln!(
                "  {} Starting node service for {}...",
                style("◆").cyan().bold(),
                node_identity.label
            );
            let _ = crate::pairing::ensure_pairing(&node_identity)?;
            crate::node_service::ensure_background(&node_identity)?;
            wait_for_socket(
                &socket_path,
                "Waiting for local node service...",
                "node service did not expose a local socket in time",
            )?;
        } else {
            eprintln!("  {} Starting daemon...", style("◆").cyan().bold());
            start::run("dev", false)?;
            wait_for_socket(
                &socket_path,
                "Waiting for daemon...",
                &format!(
                    "daemon started but socket not ready — check logs\n\
                     \x20 expected socket: {}\n\
                     \x20 tail -f {}/harmonia.log to view logs\n\
                     \x20 node-service: {}/harmonia-node-service.log",
                    socket_path.display(),
                    crate::paths::log_dir().unwrap_or_default().display(),
                    crate::paths::log_dir().unwrap_or_default().display(),
                ),
            )?;
        }
    }

    // Initialize repl-status path from data dir (same root the daemon uses).
    if let Ok(data_dir) = crate::paths::data_dir() {
        init_repl_status_path(&data_dir.to_string_lossy());
    }

    let stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("cannot connect to session service — is it running? ({})", e))?;

    let reader_stream = stream.try_clone()?;
    // Set read timeout so the reader thread can detect end-of-response
    reader_stream.set_read_timeout(Some(std::time::Duration::from_millis(300)))?;
    let mut writer_stream = stream;

    // Print banner
    print_banner(&term, &node_identity.label, &session.identity.id);
    let _ = crate::paths::append_session_event(
        session.as_ref(),
        "system",
        "session-open",
        &format!(
            "node={} socket={}",
            node_identity.label,
            socket_path.display()
        ),
    );

    // Shared state — minimal atomic flags for cross-thread coordination.
    // waiting: true while spinner is showing (reader sets false when response complete)
    // running: false triggers clean shutdown of all threads
    // reader_alive: false when reader thread exits (EOF, error, or shutdown)
    let waiting = Arc::new(AtomicBool::new(false));
    let waiting_reader = Arc::clone(&waiting);
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);
    let reader_alive = Arc::new(AtomicBool::new(true));
    let reader_alive_writer = Arc::clone(&reader_alive);
    // Response buffer: reader thread pushes lines here instead of printing directly.
    // The main thread drains and renders after the spinner is cleaned up.
    let response_buf: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let response_buf_reader = Arc::clone(&response_buf);

    // Ctrl+C
    let _ = ctrlc::set_handler(move || {
        running_ctrlc.store(false, Ordering::Relaxed);
        let _ = terminal::disable_raw_mode();
        let _ = std::io::stderr().execute(Show);
        eprintln!();
    });

    // Response reader thread — buffers response lines for the main thread to render.
    // Never writes to the terminal directly (avoids fighting the spinner for cursor control).
    // Sets waiting=false after the complete response is buffered.
    let running_reader = Arc::clone(&running);
    let session_reader = Arc::clone(&session);
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
                Ok(0) => break, // EOF — socket closed
                Ok(_) => {
                    let line = line_buf.trim_end_matches('\n').trim_end_matches('\r');
                    in_response = true;

                    // Buffer the line for the main thread to render
                    if let Ok(mut buf) = response_buf_reader.lock() {
                        buf.push(line.to_string());
                    }

                    // Log to session events
                    let _ = crate::paths::append_session_event(
                        session_reader.as_ref(),
                        "harmonia",
                        "assistant",
                        line,
                    );
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    // Read timeout — if we were in a response, it's now complete
                    if in_response {
                        in_response = false;
                        waiting_reader.store(false, Ordering::Release);
                    }
                    continue;
                }
                Err(_) => break, // real I/O error
            }
        }

        // Always clear waiting when reader exits — prevents spinner from hanging forever
        waiting_reader.store(false, Ordering::Release);
        reader_alive_writer.store(false, Ordering::Release);
    });

    let assistant_label = format!("harmonia@{}", node_identity.label);

    // Main input loop
    let exit_reason = run_input_loop(
        &mut writer_stream,
        &waiting,
        &running,
        &reader_alive,
        &response_buf,
        &assistant_label,
        &term,
        session.as_ref(),
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

    // Exit message based on WHY we exited
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

fn wait_for_socket(
    socket_path: &Path,
    status_text: &str,
    timeout_error: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;
    for _ in 0..30 {
        if socket_path.exists() {
            eprint!("\r                                     \r");
            return Ok(());
        }
        eprint!("\r  {} {}", spinner_chars[i % 10], status_text);
        let _ = std::io::stderr().flush();
        i += 1;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    eprint!("\r                                     \r");
    Err(timeout_error.into())
}

/// Drain the shared response buffer and render all lines.
/// Called from the main thread AFTER the spinner has cleaned up,
/// so there's no cursor conflict.
/// Unwrap gateway `{"text": "..."}` wrappers. Plain text passes through.
fn try_unwrap_json_text(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(t) = v.get("text").and_then(|t| t.as_str()) {
                return t.to_string();
            }
        }
    }
    line.to_string()
}

fn render_buffered_response(response_buf: &Arc<Mutex<Vec<String>>>, assistant_label: &str) {
    let lines: Vec<String> = {
        let mut buf = match response_buf.lock() {
            Ok(b) => b,
            Err(_) => return,
        };
        if buf.is_empty() {
            return;
        }
        buf.drain(..).collect()
    };

    // Print response block
    eprintln!();
    eprintln!("  {BOLD_CYAN}╭─{RESET} {DIM}{assistant_label}{RESET}");
    for line in &lines {
        let unwrapped = try_unwrap_json_text(line);
        for sub_line in unwrapped.lines() {
            print_agent_line(sub_line);
        }
    }
    eprintln!("  {BOLD_CYAN}╰─{RESET}");
    eprintln!();
    let _ = std::io::stderr().flush();
}

/// Why the session ended — determines the exit message.
enum ExitReason {
    UserQuit,       // /exit, /quit
    CtrlC,          // Ctrl+C
    ConnectionLost, // reader thread died unexpectedly
    Error(String),  // I/O or other error
}

fn run_input_loop(
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
    term: &Term,
    session: &crate::paths::SessionPaths,
) -> ExitReason {
    let mut stdout = std::io::stdout();
    let mut queued_input: Option<String> = None;
    let mut history = crate::input_history::InputHistory::load(&session.identity.node_label);
    let draft_store = crate::draft_store::DraftStore::new(session);
    let mut first_input = true;

    loop {
        if !running.load(Ordering::Relaxed) {
            // Check WHY we stopped
            return if !reader_alive.load(Ordering::Relaxed) {
                ExitReason::ConnectionLost
            } else {
                ExitReason::CtrlC
            };
        }

        // If waiting for a response, show spinner but allow typing to queue input
        if waiting.load(Ordering::Acquire) {
            let input = show_thinking_spinner_with_input(waiting, running, reader_alive);
            render_buffered_response(response_buf, assistant_label);
            if !running.load(Ordering::Relaxed) {
                continue; // re-enter loop, hit the check above
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
                session,
            ) {
                return ExitReason::Error(e.to_string());
            }
            continue;
        }

        // Read input
        let restore_draft = first_input;
        first_input = false;
        let input = match read_input_line(running, term, &mut history, &draft_store, restore_draft)
        {
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

        // Handle commands inline — detect /exit here for clean ExitReason
        if trimmed == "/exit" || trimmed == "/quit" || trimmed == "/q" {
            let _ = crate::paths::append_session_event(session, "you", "user", trimmed);
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
            session,
        ) {
            return ExitReason::Error(e.to_string());
        }
    }
}

/// Dispatch a user input string: handle commands, echo, send to daemon.
fn dispatch_input(
    trimmed: &str,
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
    term: &Term,
    stdout: &mut std::io::Stdout,
    session: &crate::paths::SessionPaths,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = crate::paths::append_session_event(session, "you", "user", trimmed);

    // Handle commands
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
            session,
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
            CommandResult::SessionText => {} // fall through to normal send
        }
    }

    // Print user message echo — extra blank line for clear separation
    eprintln!();
    eprintln!();
    eprintln!(
        "  {BOLD_GREEN}╭─{RESET} {DIM}you@{}{RESET}",
        session.identity.node_label
    );
    let user_prefix = format!("  {GREEN}│{RESET} ");
    for line in trimmed.lines() {
        print_wrapped(line, &user_prefix, &user_prefix, "");
    }
    eprintln!("  {BOLD_GREEN}╰─{RESET}");
    eprintln!();

    // Send to daemon
    send_to_daemon(writer, trimmed, waiting, running)?;
    Ok(())
}

fn read_input_line(
    running: &Arc<AtomicBool>,
    _term: &Term,
    history: &mut crate::input_history::InputHistory,
    draft: &crate::draft_store::DraftStore,
    restore_draft: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = crate::edit_buffer::EditBuffer::new();

    // Restore draft from crash/kill
    if restore_draft {
        if let Some(saved) = draft.load() {
            buf.set_text(&saved);
            eprintln!("  {DIM}Restored draft{RESET}");
        }
    }

    let mut ac_mode = AutocompleteMode::None;
    let mut box_height: u16 = 3; // initial: top border + 1 line + bottom border

    // @file autocomplete: use current working directory (like Claude Code).
    // Falls back to configured workspace if cwd fails.
    let workspace = std::env::current_dir().ok()
        .or_else(|| crate::paths::user_workspace().ok());

    terminal::enable_raw_mode()?;

    std::io::stderr().flush()?;
    std::io::stdout().flush()?;

    let (_, start_row) = cursor::position()?;
    let (_, term_h) = terminal::size()?;
    // Reserve space for initial box (3 rows) + menu; box grows downward as user types
    let total_needed: u16 = 3 + SLASH_MENU_MAX as u16;
    let max_box_row = term_h.saturating_sub(total_needed);
    let box_row = if start_row > max_box_row {
        let deficit = start_row - max_box_row;
        let mut err = std::io::stderr();
        for _ in 0..deficit {
            let _ = write!(err, "\n");
        }
        let _ = err.flush();
        queue!(err, MoveTo(0, max_box_row))?;
        err.flush()?;
        max_box_row
    } else {
        start_row
    };

    box_height = draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;

    // Helper: update autocomplete menu after input changes
    let update_menu = |input: &str,
                       cursor_pos: usize,
                       ac_mode: &mut AutocompleteMode,
                       box_row: u16,
                       box_height: u16,
                       workspace: &Option<std::path::PathBuf>| {
        if input.starts_with('/') {
            let m = slash_matches(input);
            if !m.is_empty() {
                let sel = match ac_mode {
                    AutocompleteMode::Slash { selected } => (*selected).min(m.len() - 1),
                    _ => 0,
                };
                // Clear any previous menu first
                clear_menu(box_row, box_height);
                draw_slash_menu(box_row, box_height, &m, sel);
                *ac_mode = AutocompleteMode::Slash { selected: sel };
            } else {
                *ac_mode = AutocompleteMode::None;
                clear_menu(box_row, box_height);
            }
        } else if let Some(ws) = workspace {
            if let Some((token_start, partial)) = find_at_token(input, cursor_pos) {
                let matches = file_matches(ws, &partial);
                if !matches.is_empty() {
                    let sel = match ac_mode {
                        AutocompleteMode::File { selected, .. } => {
                            (*selected).min(matches.len() - 1)
                        }
                        _ => 0,
                    };
                    clear_menu(box_row, box_height);
                    draw_file_menu(box_row, box_height, &matches, sel);
                    *ac_mode = AutocompleteMode::File {
                        selected: sel,
                        matches,
                        token_start,
                    };
                } else {
                    *ac_mode = AutocompleteMode::None;
                    clear_menu(box_row, box_height);
                }
            } else {
                *ac_mode = AutocompleteMode::None;
                clear_menu(box_row, box_height);
            }
        } else {
            *ac_mode = AutocompleteMode::None;
            clear_menu(box_row, box_height);
        }
    };

    let result = loop {
        if !running.load(Ordering::Relaxed) {
            break Ok(String::new());
        }

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    // Ctrl+C — exit
                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        clear_menu(box_row, box_height);
                        running.store(false, Ordering::Relaxed);
                        break Ok(String::new());
                    }

                    // Ctrl+D — exit on empty line
                    KeyEvent {
                        code: KeyCode::Char('d'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if buf.text().is_empty() {
                            clear_menu(box_row, box_height);
                            running.store(false, Ordering::Relaxed);
                            break Ok(String::new());
                        }
                    }

                    // Undo — Ctrl+Z
                    // Note: Cmd+Z cannot be captured in terminal apps — the terminal
                    // emulator intercepts it before it reaches the application. Ctrl+Z
                    // is the standard undo binding for all terminal applications on
                    // every platform (macOS, Linux, Windows).
                    KeyEvent {
                        code: KeyCode::Char('z'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if buf.undo() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                        }
                    }

                    // Redo — Ctrl+Y
                    KeyEvent {
                        code: KeyCode::Char('y'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if buf.redo() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                        }
                    }

                    // Ctrl+U — clear line
                    KeyEvent {
                        code: KeyCode::Char('u'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        buf.clear_line();
                        box_height = draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                        draft.save(buf.text());
                        update_menu(
                            buf.text(),
                            buf.cursor(),
                            &mut ac_mode,
                            box_row,
                            box_height,
                            &workspace,
                        );
                    }

                    // Ctrl+A — beginning of line
                    KeyEvent {
                        code: KeyCode::Char('a'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Home,
                        ..
                    } => {
                        buf.move_home();
                        box_height = draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                    }

                    // Ctrl+E — end of line
                    KeyEvent {
                        code: KeyCode::Char('e'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::End, ..
                    } => {
                        buf.move_end();
                        box_height = draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                    }

                    // Ctrl+W — delete word backward
                    KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if buf.delete_word_back() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                            update_menu(
                                buf.text(),
                                buf.cursor(),
                                &mut ac_mode,
                                box_row,
                                box_height,
                                &workspace,
                            );
                        }
                    }

                    // Up arrow — navigate menu or history
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => match &mut ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let m = slash_matches(buf.text());
                            if !m.is_empty() {
                                *selected = if *selected == 0 {
                                    m.len().min(SLASH_MENU_MAX) - 1
                                } else {
                                    *selected - 1
                                };
                                draw_slash_menu(box_row, box_height, &m, *selected);
                            }
                        }
                        AutocompleteMode::File {
                            selected, matches, ..
                        } => {
                            if !matches.is_empty() {
                                let max = matches.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected == 0 { max } else { *selected - 1 };
                                draw_file_menu(box_row, box_height, matches, *selected);
                            }
                        }
                        AutocompleteMode::None => {
                            if let Some(text) = history.navigate_up(buf.text()) {
                                let text = text.to_string();
                                buf.set_text(&text);
                                box_height =
                                    draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            }
                        }
                    },

                    // Down arrow — navigate menu or history
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => match &mut ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let m = slash_matches(buf.text());
                            if !m.is_empty() {
                                let max = m.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected >= max { 0 } else { *selected + 1 };
                                draw_slash_menu(box_row, box_height, &m, *selected);
                            }
                        }
                        AutocompleteMode::File {
                            selected, matches, ..
                        } => {
                            if !matches.is_empty() {
                                let max = matches.len().min(SLASH_MENU_MAX) - 1;
                                *selected = if *selected >= max { 0 } else { *selected + 1 };
                                draw_file_menu(box_row, box_height, matches, *selected);
                            }
                        }
                        AutocompleteMode::None => {
                            if let Some(text) = history.navigate_down() {
                                let text = text.to_string();
                                buf.set_text(&text);
                            } else {
                                buf.set_text("");
                            }
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                        }
                    },

                    // Enter — submit or select file
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        match &ac_mode {
                            AutocompleteMode::Slash { selected } => {
                                let m = slash_matches(buf.text());
                                if *selected < m.len() {
                                    buf.clear_line();
                                    for c in m[*selected].0.chars() {
                                        buf.insert_char(c);
                                    }
                                }
                            }
                            AutocompleteMode::File {
                                selected,
                                matches,
                                token_start,
                            } => {
                                // Insert selected file path, don't submit
                                if *selected < matches.len() {
                                    let fm = &matches[*selected];
                                    let replacement = if fm.is_dir {
                                        format!("@{}", fm.full_path)
                                    } else {
                                        format!("@{} ", fm.full_path)
                                    };
                                    let ts = *token_start;
                                    let cur = buf.cursor();
                                    let text = buf.text().to_string();
                                    let at_start = byte_index_for_char(&text, ts);
                                    let at_end = byte_index_for_char(&text, cur);
                                    let mut new_text = String::new();
                                    new_text.push_str(&text[..at_start]);
                                    new_text.push_str(&replacement);
                                    new_text.push_str(&text[at_end..]);
                                    let new_cursor = ts + char_len(&replacement);
                                    buf.set_text(&new_text);
                                    buf.set_cursor(new_cursor);
                                    box_height =
                                        draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                                    draft.save(buf.text());
                                    if fm.is_dir {
                                        update_menu(
                                            buf.text(),
                                            buf.cursor(),
                                            &mut ac_mode,
                                            box_row,
                                            box_height,
                                            &workspace,
                                        );
                                    } else {
                                        ac_mode = AutocompleteMode::None;
                                        clear_menu(box_row, box_height);
                                    }
                                    continue;
                                }
                            }
                            AutocompleteMode::None => {}
                        }
                        // Paste detection: if more events are immediately queued,
                        // this Enter is part of a paste — insert newline instead of submitting.
                        if event::poll(std::time::Duration::from_millis(5)).unwrap_or(false) {
                            // More input coming — this is a paste, insert newline
                            buf.insert_char('\n');
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                            update_menu(
                                buf.text(),
                                buf.cursor(),
                                &mut ac_mode,
                                box_row,
                                box_height,
                                &workspace,
                            );
                            continue;
                        }
                        // No more events — genuine submit
                        // Clear box + menu
                        let mut err = std::io::stderr();
                        let total_rows = box_height + SLASH_MENU_MAX as u16;
                        for r in 0..total_rows {
                            let _ =
                                queue!(err, MoveTo(0, box_row + r), Clear(ClearType::CurrentLine));
                        }
                        let _ = queue!(err, MoveTo(0, box_row));
                        let _ = err.flush();
                        let submitted = buf.take();
                        draft.clear();
                        history.push(&submitted);
                        history.reset_navigation();
                        break Ok(submitted);
                    }

                    // Tab — accept selected into input
                    KeyEvent {
                        code: KeyCode::Tab, ..
                    } => match &ac_mode {
                        AutocompleteMode::Slash { selected } => {
                            let sel = *selected;
                            let m = slash_matches(buf.text());
                            if sel < m.len() {
                                buf.clear_line();
                                for c in m[sel].0.chars() {
                                    buf.insert_char(c);
                                }
                                box_height =
                                    draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                                draft.save(buf.text());
                                update_menu(
                                    buf.text(),
                                    buf.cursor(),
                                    &mut ac_mode,
                                    box_row,
                                    box_height,
                                    &workspace,
                                );
                            }
                        }
                        AutocompleteMode::File {
                            selected,
                            matches,
                            token_start,
                        } => {
                            if *selected < matches.len() {
                                let fm = matches[*selected].clone();
                                let ts = *token_start;
                                let cur = buf.cursor();
                                let replacement = if fm.is_dir {
                                    format!("@{}", fm.full_path)
                                } else {
                                    format!("@{} ", fm.full_path)
                                };
                                let text = buf.text().to_string();
                                let at_start = byte_index_for_char(&text, ts);
                                let at_end = byte_index_for_char(&text, cur);
                                let mut new_text = String::new();
                                new_text.push_str(&text[..at_start]);
                                new_text.push_str(&replacement);
                                new_text.push_str(&text[at_end..]);
                                let new_cursor = ts + char_len(&replacement);
                                buf.set_text(&new_text);
                                buf.set_cursor(new_cursor);
                                box_height =
                                    draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                                draft.save(buf.text());
                                if fm.is_dir {
                                    update_menu(
                                        buf.text(),
                                        buf.cursor(),
                                        &mut ac_mode,
                                        box_row,
                                        box_height,
                                        &workspace,
                                    );
                                } else {
                                    ac_mode = AutocompleteMode::None;
                                    clear_menu(box_row, box_height);
                                }
                            }
                        }
                        AutocompleteMode::None => {}
                    },

                    // Escape — close menu
                    KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        if !matches!(ac_mode, AutocompleteMode::None) {
                            ac_mode = AutocompleteMode::None;
                            clear_menu(box_row, box_height);
                        }
                    }

                    // Backspace
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        if buf.backspace() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                            update_menu(
                                buf.text(),
                                buf.cursor(),
                                &mut ac_mode,
                                box_row,
                                box_height,
                                &workspace,
                            );
                        }
                    }

                    // Delete
                    KeyEvent {
                        code: KeyCode::Delete,
                        ..
                    } => {
                        if buf.delete() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                            draft.save(buf.text());
                            update_menu(
                                buf.text(),
                                buf.cursor(),
                                &mut ac_mode,
                                box_row,
                                box_height,
                                &workspace,
                            );
                        }
                    }

                    // Left arrow
                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    } => {
                        if buf.move_left() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                        }
                    }

                    // Right arrow
                    KeyEvent {
                        code: KeyCode::Right,
                        ..
                    } => {
                        if buf.move_right() {
                            box_height =
                                draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                        }
                    }

                    // Regular character
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        ..
                    } => {
                        buf.insert_char(c);
                        box_height = draw_prompt(buf.text(), buf.cursor(), box_row, box_height)?;
                        draft.save(buf.text());
                        update_menu(
                            buf.text(),
                            buf.cursor(),
                            &mut ac_mode,
                            box_row,
                            box_height,
                            &workspace,
                        );
                    }

                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    result
}

/// Autocomplete hints for the TUI input box.
/// Command EXECUTION is handled by the gateway (for most commands) or TUI-local
/// (for /clear, /session, /resume, /rewind, /log, /menu, /pair).
/// This list is a display aid only — the gateway is the source of truth for
/// what commands exist and how they behave.
const SLASH_COMMANDS: &[(&str, &str)] = &[
    // Gateway-handled commands
    ("/help", "Show this help"),
    ("/exit", "Exit session"),
    ("/status", "System health + subsystems"),
    ("/backends", "Active backends by category"),
    ("/tools", "Registered tools"),
    ("/frontends", "Setup and pair frontends"),
    ("/chronicle", "Chronicle event query"),
    ("/metrics", "Runtime metrics"),
    ("/security", "Security posture"),
    ("/feedback", "Response style feedback"),
    ("/identity", "Agent identity"),
    ("/wallet", "Wallet/vault status"),
    ("/policies", "Channel sender policies"),
    ("/auto", "Intelligent routing (default)"),
    ("/eco", "Cost-optimized routing"),
    ("/premium", "Quality-optimized routing"),
    ("/free", "Zero-cost routing (local CLI only)"),
    ("/route", "Current routing status"),
    // TUI-local commands
    ("/clear", "New session, clear screen"),
    ("/resume", "Resume a past session"),
    ("/rewind", "Rewind to a previous turn"),
    ("/session", "Current session info"),
    ("/log", "Recent log entries"),
];

const SLASH_MENU_MAX: usize = 8;

fn slash_matches(partial: &str) -> Vec<(&'static str, &'static str)> {
    let query = partial.to_lowercase();
    SLASH_COMMANDS
        .iter()
        .filter(|(cmd, desc)| {
            // Match by prefix on command OR substring on command+description
            cmd.starts_with(partial)
                || cmd[1..].contains(&query[1..].to_string())
                || desc.to_lowercase().contains(&query[1..])
        })
        .copied()
        .collect()
}

/// Draw the command palette below the input box — like Claude Code.
/// Left: command name. Right-aligned: short description.
fn draw_slash_menu(box_row: u16, box_height: u16, matches: &[(&str, &str)], selected: usize) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    let (term_w, _) = terminal::size().unwrap_or((80, 24));
    let visible = matches.len().min(SLASH_MENU_MAX);
    for i in 0..SLASH_MENU_MAX {
        let row = box_row + box_height + i as u16;
        let _ = queue!(err, MoveTo(0, row), Clear(ClearType::CurrentLine));
        if i < visible {
            let (cmd, desc) = matches[i];
            // Right-align the description
            let cmd_width = cmd.len() + 4; // "  > /cmd"
            let desc_width = desc.len();
            let padding = if term_w as usize > cmd_width + desc_width + 4 {
                term_w as usize - cmd_width - desc_width - 4
            } else {
                2
            };
            let spaces = " ".repeat(padding);
            if i == selected {
                let _ = queue!(
                    err,
                    Print(format!(
                        "  {BOLD_CYAN}▸ {cmd}{RESET}{spaces}{DIM}{desc}{RESET}"
                    ))
                );
            } else {
                let _ = queue!(err, Print(format!("  {DIM}  {cmd}{spaces}{desc}{RESET}")));
            }
        }
    }
    let _ = queue!(err, RestorePosition);
    let _ = err.flush();
}

/// Draw the file autocomplete dropdown menu below the input box.
fn draw_file_menu(box_row: u16, box_height: u16, matches: &[FileMatch], selected: usize) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    let visible = matches.len().min(SLASH_MENU_MAX);
    for i in 0..SLASH_MENU_MAX {
        let row = box_row + box_height + i as u16;
        let _ = queue!(err, MoveTo(0, row), Clear(ClearType::CurrentLine));
        if i < visible {
            let fm = &matches[i];
            if i == selected {
                let _ = queue!(
                    err,
                    Print(format!("  {BOLD_CYAN}  {}{RESET}", fm.display))
                );
            } else {
                let _ = queue!(err, Print(format!("  {DIM}  {}{RESET}", fm.display)));
            }
        }
    }
    let _ = queue!(err, RestorePosition);
    let _ = err.flush();
}

fn clear_menu(box_row: u16, box_height: u16) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    for i in 0..SLASH_MENU_MAX {
        let _ = queue!(
            err,
            MoveTo(0, box_row + box_height + i as u16),
            Clear(ClearType::CurrentLine)
        );
    }
    let _ = queue!(err, RestorePosition);
    let _ = err.flush();
}

fn term_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

fn char_len(input: &str) -> usize {
    input.chars().count()
}

fn byte_index_for_char(input: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    input
        .char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| input.len())
}

fn display_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

// ── Autocomplete types ───────────────────────────────────────────────

#[derive(Clone)]
struct FileMatch {
    display: String,
    full_path: String,
    is_dir: bool,
}

enum AutocompleteMode {
    None,
    Slash {
        selected: usize,
    },
    File {
        selected: usize,
        matches: Vec<FileMatch>,
        token_start: usize,
    },
}

/// Find the @token at or before cursor_pos.
/// Returns (token_start_char_index, partial_text_after_@) or None.
fn find_at_token(input: &str, cursor_pos: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let pos = cursor_pos.min(chars.len());
    // Scan backwards from cursor looking for @
    let mut i = pos;
    while i > 0 {
        i -= 1;
        if chars[i] == '@' {
            // Check no whitespace between @ and cursor
            let partial: String = chars[i + 1..pos].iter().collect();
            if partial.contains(char::is_whitespace) {
                return None;
            }
            return Some((i, partial));
        }
        if chars[i].is_whitespace() {
            return None;
        }
    }
    None
}

/// File completion: navigate into directories as user types path.
/// @             → workspace root contents
/// @harm         → items starting with "harm" at root
/// @harmoniis/   → contents of harmoniis/
/// @harmoniis/agent/ → contents of harmoniis/agent/
/// Each level shows direct children. Selecting a dir appends / and shows next level.
fn file_matches(workspace: &Path, partial: &str) -> Vec<FileMatch> {
    let (parent_dir, prefix) = if let Some(slash_pos) = partial.rfind('/') {
        (&partial[..=slash_pos], &partial[slash_pos + 1..])
    } else {
        ("", partial)
    };

    let search_dir = if parent_dir.is_empty() {
        workspace.to_path_buf()
    } else {
        workspace.join(parent_dir)
    };

    let prefix_lower = prefix.to_lowercase();
    let Ok(entries) = std::fs::read_dir(&search_dir) else {
        return Vec::new();
    };

    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut items: Vec<_> = entries.flatten().collect();
    items.sort_by_key(|e| e.file_name());

    for entry in items {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "target" || name == "node_modules" { continue; }
        if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) { continue; }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let full_path = format!("{}{}{}", parent_dir, name, if is_dir { "/" } else { "" });
        let display = full_path.clone();
        let fm = FileMatch { display, full_path, is_dir };
        if is_dir { dirs.push(fm); } else { files.push(fm); }
    }
    dirs.append(&mut files);
    dirs.truncate(SLASH_MENU_MAX);
    dirs
}

// ── Multiline wrapping ───────────────────────────────────────────────

/// Wrap input text into visual lines that fit within `view_width`.
/// Returns (lines, cursor_line, cursor_col).
fn wrap_input(input: &str, cursor_pos: usize, view_width: usize) -> (Vec<String>, usize, usize) {
    if view_width == 0 {
        return (vec![String::new()], 0, 0);
    }

    let chars: Vec<char> = input.chars().collect();
    let cursor_pos = cursor_pos.min(chars.len());

    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut current_width: usize = 0;
    let mut cursor_line: usize = 0;
    let mut cursor_col: usize = 0;

    for (i, &ch) in chars.iter().enumerate() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);

        if current_width + ch_width > view_width {
            // Start a new visual line
            lines.push(std::mem::take(&mut current_line));
            current_width = 0;
        }

        if i == cursor_pos {
            cursor_line = lines.len();
            cursor_col = current_width;
        }

        current_line.push(ch);
        current_width += ch_width;
    }

    // Cursor at end of input
    if cursor_pos == chars.len() {
        if current_width >= view_width && !current_line.is_empty() {
            lines.push(std::mem::take(&mut current_line));
            cursor_line = lines.len();
            cursor_col = 0;
            // Push the empty line where cursor sits
            lines.push(String::new());
        } else {
            cursor_line = lines.len();
            cursor_col = current_width;
            lines.push(current_line);
        }
    } else {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    // Cap at MAX_INPUT_LINES, keeping cursor line visible
    if lines.len() > MAX_INPUT_LINES {
        // Determine the window of lines to show
        let total = lines.len();
        let start = if cursor_line < MAX_INPUT_LINES {
            0
        } else if cursor_line >= total {
            total.saturating_sub(MAX_INPUT_LINES)
        } else {
            (cursor_line + 1).saturating_sub(MAX_INPUT_LINES)
        };
        lines = lines[start..start + MAX_INPUT_LINES].to_vec();
        cursor_line -= start;
    }

    (lines, cursor_line, cursor_col)
}

/// Draw the multiline input box and position the cursor.
/// Returns total box height (2 + content_lines).
/// `prev_height` is used to clear leftover rows when the box shrinks.
fn draw_prompt(
    input: &str,
    cursor_pos: usize,
    box_row: u16,
    prev_height: u16,
) -> Result<u16, Box<dyn std::error::Error>> {
    let mut err = std::io::stderr();
    let width = term_width() as usize;
    let inner = if width > 8 { width - 6 } else { 40 };
    let view_w = if inner > 2 { inner - 2 } else { inner };

    let (lines, cursor_line, cursor_col) = wrap_input(input, cursor_pos, view_w);
    let num_lines = lines.len();
    let box_height = 2 + num_lines as u16;
    let bar = "─".repeat(inner);

    queue!(err, Hide)?;

    // Top border
    queue!(
        err,
        MoveTo(0, box_row),
        Clear(ClearType::CurrentLine),
        Print(format!("  {DIM}╭{bar}╮{RESET}"))
    )?;

    // Content lines
    for (i, line) in lines.iter().enumerate() {
        let pad = " ".repeat(view_w.saturating_sub(display_width(line)));
        queue!(
            err,
            MoveTo(0, box_row + 1 + i as u16),
            Clear(ClearType::CurrentLine),
            Print(format!(
                "  {DIM}│{RESET} {BOLD_WHITE}{line}{RESET}{pad} {DIM}│{RESET}"
            ))
        )?;
    }

    // Bottom border
    let bottom_row = box_row + 1 + num_lines as u16;
    queue!(
        err,
        MoveTo(0, bottom_row),
        Clear(ClearType::CurrentLine),
        Print(format!("  {DIM}╰{bar}╯{RESET}"))
    )?;

    // Clear leftover rows when box shrinks
    if box_height < prev_height {
        for r in box_height..prev_height {
            queue!(err, MoveTo(0, box_row + r), Clear(ClearType::CurrentLine))?;
        }
    }

    // Position cursor — steady block style
    queue!(
        err,
        SetCursorStyle::SteadyBlock,
        Show,
        MoveTo(4 + cursor_col as u16, box_row + 1 + cursor_line as u16)
    )?;

    err.flush()?;
    Ok(box_height)
}

/// Show thinking spinner with a persistent input box below it.
/// The input box stays visible and functional — user can type ahead.
/// Returns Some(text) if the user submitted input during thinking, None otherwise.
/// Empty-enter (no text) is silently ignored.
fn show_thinking_spinner_with_input(
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
) -> Option<String> {
    let dots = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;
    let mut input_buf = String::new();
    let mut cursor_pos: usize = 0;
    let started = std::time::Instant::now();
    // Adaptive timeout: check live status — if "all models unavailable", break early.
    // Base timeout is generous (120s for thinking models), but cascade failures break fast.
    const SPINNER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

    let _ = terminal::enable_raw_mode();

    // Reserve space: 1 row for spinner + 3 rows for input box (top border, content, bottom border)
    let (_, term_h) = terminal::size().unwrap_or((80, 24));
    let (_, cur_row) = cursor::position().unwrap_or((0, 0));
    let needed: u16 = 4; // spinner + 3-line box minimum
    let spinner_row = if cur_row + needed >= term_h {
        // Scroll up to make space
        let deficit = (cur_row + needed) - term_h + 1;
        let mut err = std::io::stderr();
        for _ in 0..deficit {
            let _ = write!(err, "\n");
        }
        let _ = err.flush();
        term_h.saturating_sub(needed).saturating_sub(1)
    } else {
        cur_row
    };
    let box_row = spinner_row + 1;
    let mut box_height: u16 = 3;

    // Initial draw
    let _ = draw_spinner_line(spinner_row, dots[0], false);
    let _ = draw_prompt(&input_buf, cursor_pos, box_row, box_height);

    while waiting.load(Ordering::Acquire) && running.load(Ordering::Relaxed) {
        // Break if reader thread has died (socket closed, daemon unreachable)
        if !reader_alive.load(Ordering::Acquire) {
            clear_spinner_and_box(spinner_row, box_row, box_height);
            let _ = terminal::disable_raw_mode();
            waiting.store(false, Ordering::Release);
            eprintln!("\n  {RED}✗{RESET} No response — daemon connection lost.");
            eprintln!();
            return if !input_buf.trim().is_empty() {
                Some(input_buf)
            } else {
                None
            };
        }

        // Early break: if REPL reports all models unavailable, don't wait.
        {
            let status = read_repl_status();
            if status.contains("all models unavailable") {
                clear_spinner_and_box(spinner_row, box_row, box_height);
                let _ = terminal::disable_raw_mode();
                waiting.store(false, Ordering::Release);
                eprintln!("\n  {RED}✗{RESET} All models unavailable — check network/providers.");
                eprintln!();
                return if !input_buf.trim().is_empty() { Some(input_buf) } else { None };
            }
        }
        // Break if we've been waiting too long
        if started.elapsed() > SPINNER_TIMEOUT {
            clear_spinner_and_box(spinner_row, box_row, box_height);
            let _ = terminal::disable_raw_mode();
            waiting.store(false, Ordering::Release);
            eprintln!(
                "\n  {YELLOW}!{RESET} Response timeout ({}s) — daemon may be unresponsive.",
                SPINNER_TIMEOUT.as_secs()
            );
            eprintln!();
            return if !input_buf.trim().is_empty() {
                Some(input_buf)
            } else {
                None
            };
        }

        // Animate spinner
        let _ = draw_spinner_line(spinner_row, dots[i % dots.len()], !input_buf.is_empty());

        // Redraw input box with current content
        if let Ok(h) = draw_prompt(&input_buf, cursor_pos, box_row, box_height) {
            box_height = h;
        }
        i += 1;

        // Poll for keyboard input (80ms = spinner frame rate)
        if event::poll(std::time::Duration::from_millis(80)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match key {
                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        clear_spinner_and_box(spinner_row, box_row, box_height);
                        running.store(false, Ordering::Relaxed);
                        let _ = terminal::disable_raw_mode();
                        let _ = std::io::stderr().execute(Show);
                        return None;
                    }
                    KeyEvent {
                        code: KeyCode::Char(ch),
                        modifiers,
                        ..
                    } if !modifiers.contains(KeyModifiers::CONTROL) => {
                        let byte_idx = byte_index_for_char(&input_buf, cursor_pos);
                        input_buf.insert(byte_idx, ch);
                        cursor_pos += 1;
                    }
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            let byte_idx = byte_index_for_char(&input_buf, cursor_pos);
                            let end_idx = byte_index_for_char(&input_buf, cursor_pos + 1);
                            input_buf.drain(byte_idx..end_idx);
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Right,
                        ..
                    } => {
                        if cursor_pos < char_len(&input_buf) {
                            cursor_pos += 1;
                        }
                    }
                    KeyEvent {
                        code: KeyCode::Home,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('a'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        cursor_pos = 0;
                    }
                    KeyEvent {
                        code: KeyCode::End, ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('e'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        cursor_pos = char_len(&input_buf);
                    }
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        if !input_buf.trim().is_empty() {
                            // User submitted — clear spinner + box, return text
                            clear_spinner_and_box(spinner_row, box_row, box_height);
                            let _ = terminal::disable_raw_mode();
                            return Some(input_buf);
                        }
                        // Empty enter: silently ignore
                    }
                    KeyEvent {
                        code: KeyCode::Char('u'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        input_buf.clear();
                        cursor_pos = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    // Thinking finished — clear spinner line, leave box area clean
    clear_spinner_and_box(spinner_row, box_row, box_height);
    let _ = terminal::disable_raw_mode();

    if !input_buf.trim().is_empty() {
        Some(input_buf)
    } else {
        None
    }
}

/// Cached state root — set once at session start, used by spinner.
static REPL_STATUS_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

fn init_repl_status_path(state_root: &str) {
    let _ = REPL_STATUS_PATH.set(std::path::PathBuf::from(state_root).join("repl-status.txt"));
}

fn read_repl_status() -> String {
    REPL_STATUS_PATH.get()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

fn draw_spinner_line(row: u16, dot: &str, has_queued: bool) {
    let mut err = std::io::stderr();
    let live_status = read_repl_status();
    let status_text = if !live_status.trim().is_empty() {
        live_status.trim().to_string()
    } else {
        "thinking...".to_string()
    };
    let status = if has_queued {
        format!("  {CYAN}{dot}{RESET} {DIM}{status_text}{RESET}  {DIM}(queued){RESET}")
    } else {
        format!("  {CYAN}{dot}{RESET} {DIM}{status_text}{RESET}")
    };
    let _ = queue!(
        err,
        SavePosition,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(status),
        RestorePosition
    );
    let _ = err.flush();
}

fn clear_spinner_and_box(spinner_row: u16, box_row: u16, box_height: u16) {
    let mut err = std::io::stderr();
    // Clear spinner line
    let _ = queue!(err, MoveTo(0, spinner_row), Clear(ClearType::CurrentLine));
    // Clear box lines
    for r in 0..box_height {
        let _ = queue!(err, MoveTo(0, box_row + r), Clear(ClearType::CurrentLine));
    }
    let _ = queue!(err, MoveTo(0, spinner_row));
    let _ = err.flush();
}

fn print_agent_line(line: &str) {
    // Prefix: "  │ " = 4 visible columns (2 margin + border + space)
    let prefix_col = "│";
    let cont_prefix = format!("  {CYAN}{prefix_col}{RESET} ");

    if line.starts_with("[ERROR]") || line.starts_with("Error:") {
        print_wrapped(
            line,
            &format!("  {RED}{prefix_col}{RESET} {RED}"),
            &format!("  {RED}{prefix_col}{RESET} {RED}"),
            RED,
        );
    } else if line.starts_with("[WARN]") || line.starts_with("Warning:") {
        print_wrapped(
            line,
            &format!("  {YELLOW}{prefix_col}{RESET} {YELLOW}"),
            &format!("  {YELLOW}{prefix_col}{RESET} {YELLOW}"),
            YELLOW,
        );
    } else if line.starts_with("[DEBUG]") {
        print_wrapped(
            line,
            &format!("  {DIM}{prefix_col} "),
            &format!("  {DIM}{prefix_col} "),
            DIM,
        );
    } else if line.starts_with("```") {
        eprintln!("  {CYAN}{prefix_col}{RESET} {DIM}{line}{RESET}");
    } else if line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") {
        print_wrapped(
            line,
            &format!("  {CYAN}{prefix_col}{RESET} {BOLD_WHITE}"),
            &cont_prefix,
            "",
        );
    } else if line.starts_with("- ") || line.starts_with("* ") {
        print_wrapped(
            &line[2..],
            &format!("  {CYAN}{prefix_col}{RESET} {CYAN}•{RESET} "),
            &format!("  {CYAN}{prefix_col}{RESET}   "),
            "",
        );
    } else if line.starts_with("> ") {
        print_wrapped(
            &line[2..],
            &format!("  {CYAN}{prefix_col}{RESET} {DIM}▎"),
            &format!("  {CYAN}{prefix_col}{RESET} {DIM} "),
            DIM,
        );
    } else {
        print_wrapped(line, &cont_prefix, &cont_prefix, "");
    }
}

/// Print text with word-wrapping so continuation lines stay inside the fence.
/// `first_prefix` is printed before the first visual line.
/// `cont_prefix` is printed before each continuation line.
/// `color` is applied to the text content (empty string = no extra color).
fn print_wrapped(text: &str, first_prefix: &str, cont_prefix: &str, color: &str) {
    let tw = term_width() as usize;
    // Visible prefix: "  │ " = 4 cols left, plus 4 cols right margin
    let margin = 8; // 4 left + 4 right
    let content_w = if tw > margin + 2 { tw - margin } else { 40 };

    if text.is_empty() {
        eprintln!("{first_prefix}{RESET}");
        return;
    }

    let mut remaining = text;
    let mut first = true;
    while !remaining.is_empty() {
        let pfx = if first { first_prefix } else { cont_prefix };
        first = false;

        // Take up to content_w characters
        let take: String = remaining.chars().take(content_w).collect();
        let taken_chars = take.chars().count();

        // Advance remaining past what we took
        let byte_end = remaining
            .char_indices()
            .nth(taken_chars)
            .map(|(i, _)| i)
            .unwrap_or(remaining.len());
        remaining = &remaining[byte_end..];

        if color.is_empty() {
            eprintln!("{pfx}{take}{RESET}");
        } else {
            eprintln!("{pfx}{color}{take}{RESET}");
        }
    }
}

fn print_banner(term: &Term, node_label: &str, session_id: &str) {
    let width = term.size().1 as usize;
    let bar_width = width.min(56);
    let bar = "─".repeat(bar_width);

    eprintln!();

    // ASCII art logo in cyan
    for line in LOGO.lines() {
        if !line.is_empty() {
            eprintln!("{BOLD_CYAN}{}{RESET}", line);
        }
    }

    eprintln!();
    eprintln!(
        "  {DIM}v{VERSION} — Distributed evolutionary homoiconic self-improving agent{RESET}"
    );
    eprintln!(
        "  {DIM}node:{RESET} {CYAN}{node_label}{RESET}  {DIM}session:{RESET} {CYAN}{session_id}{RESET}"
    );
    eprintln!();
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!();
    eprintln!("  {DIM}Type a message to continue this session with Harmonia.{RESET}");
    eprintln!(
        "  {DIM}Use {RESET}{CYAN}/help{RESET}{DIM} for commands, {RESET}{CYAN}/exit{RESET}{DIM} to quit.{RESET}"
    );
    eprintln!();
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!();
}

fn send_to_daemon(
    writer: &mut UnixStream,
    message: &str,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = writeln!(writer, "{}", message) {
        eprintln!("  {RED}✗{RESET} Connection lost: {}", e);
        running.store(false, Ordering::Relaxed);
        return Err(e.into());
    }
    let _ = writer.flush();
    waiting.store(true, Ordering::Release);
    Ok(())
}

enum CommandResult {
    Handled,
    Quit,
    SendToAgent(String),
    SessionText, // not a command, send as a regular session message
}

fn handle_command(
    cmd: &str,
    term: &Term,
    stdout: &mut std::io::Stdout,
    _writer: &mut UnixStream,
    _waiting: &Arc<AtomicBool>,
    _running: &Arc<AtomicBool>,
    _reader_alive: &Arc<AtomicBool>,
    _response_buf: &Arc<Mutex<Vec<String>>>,
    _assistant_label: &str,
    session: &crate::paths::SessionPaths,
) -> CommandResult {
    let base = cmd.split_whitespace().next().unwrap_or("");
    match base {
        // ── TUI-local commands (session-local / terminal-local) ──
        "/session" => {
            print_session_summary(session);
            CommandResult::Handled
        }
        "/clear" | "/cls" => {
            // Start a fresh session and clear the screen
            let node = match crate::paths::current_node_identity() {
                Ok(n) => n,
                Err(_) => {
                    let _ = term.clear_screen();
                    print_banner(term, &session.identity.node_label, &session.identity.id);
                    return CommandResult::Handled;
                }
            };
            match crate::paths::create_session(&node) {
                Ok(new_session) => {
                    let _ = term.clear_screen();
                    print_banner(
                        term,
                        &new_session.identity.node_label,
                        &new_session.identity.id,
                    );
                    eprintln!("  {DIM}New session started.{RESET}");
                    eprintln!();
                }
                Err(_) => {
                    let _ = term.clear_screen();
                    print_banner(term, &session.identity.node_label, &session.identity.id);
                }
            }
            CommandResult::Handled
        }
        "/log" | "/logs" => {
            print_recent_log();
            CommandResult::Handled
        }
        "/quit" | "/exit" | "/q" => CommandResult::Quit,

        // ── Rewind (like git reset) ──
        "/rewind" => match run_rewind_flow(stdout, session, term) {
            Ok(true) => CommandResult::Handled,
            Ok(false) => CommandResult::Handled,
            Err(e) => {
                eprintln!("\n  {RED}Rewind error: {}{RESET}", e);
                CommandResult::Handled
            }
        },

        // ── Session management ──
        "/resume" => {
            let node = match crate::paths::current_node_identity() {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("\n  {RED}Error: {}{RESET}", e);
                    return CommandResult::Handled;
                }
            };
            match run_resume_flow(stdout, session, &node) {
                Ok(true) => CommandResult::Handled, // history replayed, continue in session
                Ok(false) => CommandResult::Handled,
                Err(e) => {
                    eprintln!("\n  {RED}Resume error: {}{RESET}", e);
                    CommandResult::Handled
                }
            }
        }

        // ── Interactive menu (TUI navigation aid) ──
        "/menu" | "/m" => {
            eprintln!("\n  {DIM}Use slash commands directly. /help for available commands.{RESET}\n");
            CommandResult::Handled
        }

        // ── Device pairing (TUI-local interactive flow) ──
        "/pair" | "/link" => {
            eprintln!(
                "\n  {DIM}Use /frontends from /menu. /pair is a compatibility alias.{RESET}\n"
            );
            match crate::paths::current_node_identity() {
                Ok(node_identity) => {
                    if let Err(e) =
                        crate::frontend_pairing::run_pairing_menu(stdout, &node_identity)
                    {
                        eprintln!("\n  {RED}Frontend error: {}{RESET}", e);
                    }
                }
                Err(e) => {
                    eprintln!("\n  {RED}Cannot load node identity: {}{RESET}", e);
                }
            }
            CommandResult::Handled
        }

        // ── Everything else: delegate to the gateway ──
        // Gateway handles: /help, /status, /backends, /providers, /frontends,
        // /tools, /chronicle, /metrics, /security, /identity, /feedback,
        // /wallet, /policies, /auto, /eco, /premium, /free, /route, /exit
        // Unknown commands pass through to the agent as prompts.
        _ if base.starts_with('/') => {
            // Remap TUI aliases to gateway-canonical names
            let canonical = match base {
                "/h" | "/?" => "/help".to_string(),
                "/providers" => {
                    // /providers -> /backends (gateway name)
                    cmd.replacen("/providers", "/backends", 1)
                }
                _ => cmd.to_string(),
            };
            CommandResult::SendToAgent(canonical)
        }

        _ => CommandResult::SessionText,
    }
}


/// Returns true if session was switched (caller should reconnect), false if cancelled.
fn run_resume_flow(
    stdout: &mut std::io::Stdout,
    session: &crate::paths::SessionPaths,
    node: &crate::paths::NodeIdentity,
) -> Result<bool, Box<dyn std::error::Error>> {
    let label = &node.label;
    let sessions = crate::paths::list_sessions(label)?;

    if sessions.is_empty() {
        eprintln!("\n  {DIM}No past sessions found.{RESET}\n");
        return Ok(false);
    }

    let items: Vec<crate::menus::MenuItem> = sessions
        .iter()
        .map(|s| {
            let ts = crate::paths::format_timestamp_ms(s.updated_at_ms);
            let created = crate::paths::format_timestamp_ms(s.created_at_ms);
            let current = if s.id == session.identity.id {
                " (current)"
            } else {
                ""
            };
            crate::menus::MenuItem::new(
                &format!("{}{}", ts, current),
                &s.id,
                &format!("{} events, created {}", s.event_count, created),
            )
        })
        .collect();

    match crate::menus::interactive_select(stdout, "Resume Session", &items)? {
        crate::menus::MenuAction::Command(selected_id) => {
            // Load the selected session and replay its history
            let resumed = crate::paths::resume_session(node, &selected_id)?;
            eprintln!();
            replay_session_history(&resumed, label);
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// A conversation turn: one user message + the assistant response.
struct Turn {
    index: usize,              // turn number (1-based)
    user_text: String,         // what the user said
    assistant_preview: String, // first line of response
    event_end: usize,          // index in events list where this turn ends
}

/// Rewind the conversation to a previous turn.
/// Like git reset: everything after the chosen turn is removed.
/// The session events file is truncated and the conversation replays.
fn run_rewind_flow(
    stdout: &mut std::io::Stdout,
    session: &crate::paths::SessionPaths,
    term: &Term,
) -> Result<bool, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(&session.events_path)?;
    let events: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if events.is_empty() {
        eprintln!("\n  {DIM}No conversation to rewind.{RESET}\n");
        return Ok(false);
    }

    // Parse events into turns
    let mut turns: Vec<Turn> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        if let Ok(ev) = serde_json::from_str::<serde_json::Value>(events[i]) {
            let actor = ev.get("actor").and_then(|v| v.as_str()).unwrap_or("");
            let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let text = ev.get("text").and_then(|v| v.as_str()).unwrap_or("");

            if actor == "you" && kind == "user" && !text.starts_with('/') {
                let user_text = text.to_string();
                // Collect assistant response lines
                let mut assistant_lines = Vec::new();
                let mut j = i + 1;
                while j < events.len() {
                    if let Ok(rev) = serde_json::from_str::<serde_json::Value>(events[j]) {
                        let ra = rev.get("actor").and_then(|v| v.as_str()).unwrap_or("");
                        let rk = rev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                        let rt = rev.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        if ra == "harmonia" && rk == "assistant" {
                            assistant_lines.push(rt.to_string());
                            j += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                let preview = if assistant_lines.is_empty() {
                    "(no response)".to_string()
                } else {
                    let first = &assistant_lines[0];
                    if first.len() > 60 {
                        format!("{}...", &first[..60])
                    } else {
                        first.clone()
                    }
                };
                turns.push(Turn {
                    index: turns.len() + 1,
                    user_text,
                    assistant_preview: preview,
                    event_end: j, // events[0..j] = everything up to and including this turn
                });
                i = j;
                continue;
            }
        }
        i += 1;
    }

    if turns.is_empty() {
        eprintln!("\n  {DIM}No conversation turns to rewind to.{RESET}\n");
        return Ok(false);
    }

    // Build menu items
    let items: Vec<crate::menus::MenuItem> = turns
        .iter()
        .rev() // most recent first
        .map(|t| {
            let clipped_user = if t.user_text.len() > 50 {
                format!("{}...", &t.user_text[..50])
            } else {
                t.user_text.clone()
            };
            crate::menus::MenuItem::new(
                &format!("Turn {} — {}", t.index, clipped_user),
                &t.index.to_string(),
                &t.assistant_preview,
            )
        })
        .collect();

    match crate::menus::interactive_select(stdout, "Rewind to turn", &items)? {
        crate::menus::MenuAction::Command(selected) => {
            let turn_num: usize = selected.parse().unwrap_or(0);
            if let Some(turn) = turns.iter().find(|t| t.index == turn_num) {
                // Truncate events file to this turn
                let kept: Vec<&str> = events[..turn.event_end].to_vec();
                let new_content = kept.join("\n") + "\n";
                std::fs::write(&session.events_path, &new_content)?;

                // Clear screen and replay
                let _ = term.clear_screen();
                let label = &session.identity.node_label;
                print_banner(term, label, &session.identity.id);
                replay_session_history(session, label);

                let removed = turns.len() - turn_num;
                eprintln!(
                    "  {BOLD_CYAN}◆{RESET} Rewound to turn {}. {} turn{} removed.",
                    turn_num,
                    removed,
                    if removed == 1 { "" } else { "s" }
                );
                eprintln!();
                Ok(true)
            } else {
                eprintln!("\n  {DIM}Turn not found.{RESET}\n");
                Ok(false)
            }
        }
        _ => Ok(false),
    }
}

/// Replay session history on connect — shows the conversation so far.
/// Renders user and assistant messages in the same format as live responses.
fn replay_session_history(session: &crate::paths::SessionPaths, node_label: &str) {
    let content = match std::fs::read_to_string(&session.events_path) {
        Ok(c) if !c.is_empty() => c,
        _ => return,
    };

    let events: Vec<serde_json::Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();

    if events.is_empty() {
        return;
    }

    // Find the last session-open marker — only replay from there
    let replay_start = events
        .iter()
        .rposition(|e| e.get("kind").and_then(|v| v.as_str()) == Some("session-open"))
        .map(|i| i + 1) // skip the session-open event itself
        .unwrap_or(0);

    let replay_events = &events[replay_start..];
    if replay_events.is_empty() {
        return;
    }

    eprintln!("  {DIM}── session history ──{RESET}");
    eprintln!();

    // Group consecutive messages by actor for clean block rendering
    let mut i = 0;
    while i < replay_events.len() {
        let event = &replay_events[i];
        let actor = event.get("actor").and_then(|v| v.as_str()).unwrap_or("");
        let kind = event.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let text = event.get("text").and_then(|v| v.as_str()).unwrap_or("");

        match (actor, kind) {
            ("you", "user") => {
                eprintln!("  {BOLD_GREEN}╭─{RESET} {DIM}you@{node_label}{RESET}");
                eprintln!("  {GREEN}│{RESET} {}", text);
                eprintln!("  {BOLD_GREEN}╰─{RESET}");
                eprintln!();
            }
            ("harmonia", "assistant") => {
                eprintln!("  {BOLD_CYAN}╭─{RESET} {DIM}harmonia@{node_label}{RESET}");
                // Collect consecutive assistant lines into one block
                let mut j = i;
                while j < replay_events.len() {
                    let ev = &replay_events[j];
                    if ev.get("actor").and_then(|v| v.as_str()) != Some("harmonia")
                        || ev.get("kind").and_then(|v| v.as_str()) != Some("assistant")
                    {
                        break;
                    }
                    let line = ev.get("text").and_then(|v| v.as_str()).unwrap_or("");
                    eprintln!("  {CYAN}│{RESET} {}", line);
                    j += 1;
                }
                eprintln!("  {BOLD_CYAN}╰─{RESET}");
                eprintln!();
                i = j;
                continue;
            }
            _ => {} // skip system events
        }
        i += 1;
    }

    eprintln!("  {DIM}── end of history ──{RESET}");
    eprintln!();
}


fn print_session_summary(session: &crate::paths::SessionPaths) {
    eprintln!();
    eprintln!("  {BOLD_CYAN}◆{RESET} {BOLD}Session{RESET}");
    eprintln!("  {DIM}──────────────────────────────────────{RESET}");
    eprintln!("  {CYAN}id{RESET}          {}", session.identity.id);
    eprintln!("  {CYAN}node{RESET}        {}", session.identity.node_label);
    eprintln!(
        "  {CYAN}role{RESET}        {}",
        session.identity.node_role.as_str()
    );
    eprintln!(
        "  {CYAN}profile{RESET}     {}",
        session.identity.install_profile.as_str()
    );
    eprintln!("  {CYAN}path{RESET}        {}", session.dir.display());
    eprintln!(
        "  {CYAN}events{RESET}      {}",
        session.events_path.display()
    );
    eprintln!();
}

fn print_recent_log() {
    let log_path = match crate::paths::log_path() {
        Ok(p) => p,
        Err(_) => return,
    };

    eprintln!();
    eprintln!("  {BOLD_CYAN}◆{RESET} {BOLD}Recent Logs{RESET}");
    eprintln!("  {DIM}──────────────────────────────────{RESET}");

    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(15);
            for line in &lines[start..] {
                if line.contains("[ERROR]") {
                    eprintln!("  {RED}│{RESET} {RED}{}{RESET}", line);
                } else if line.contains("[WARN]") {
                    eprintln!("  {YELLOW}│{RESET} {YELLOW}{}{RESET}", line);
                } else if line.contains("[DEBUG]") {
                    eprintln!("  {DIM}│ {}{RESET}", line);
                } else if line.contains("[INFO]") {
                    eprintln!("  {CYAN}│{RESET} {}", line);
                } else {
                    eprintln!("  {DIM}│{RESET} {DIM}{}{RESET}", line);
                }
            }
        }
        Err(_) => {
            eprintln!("  {DIM}│ No log file found.{RESET}");
        }
    }
    eprintln!("  {DIM}──────────────────────────────────{RESET}");
    eprintln!();
}
