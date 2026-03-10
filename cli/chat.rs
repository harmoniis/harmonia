use crate::start;
use console::{style, Term};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal,
    ExecutableCommand,
};
use std::io::{BufRead, BufReader, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const LOGO: &str = r#"
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
"#;

// ── Colors ────────────────────────────────────────────────────────────

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
    let socket_path = crate::paths::socket_path()?;

    if !socket_path.exists() {
        eprintln!(
            "  {} Starting daemon...",
            style("◆").cyan().bold()
        );
        start::run("dev", false)?;

        // Wait for socket
        let spinner_chars = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mut i = 0;
        for _ in 0..30 {
            if socket_path.exists() {
                break;
            }
            eprint!("\r  {} Waiting for daemon...", spinner_chars[i % 10]);
            let _ = std::io::stderr().flush();
            i += 1;
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        eprint!("\r                                     \r");

        if !socket_path.exists() {
            return Err("daemon started but socket not ready — check harmonia.log".into());
        }
    }

    let stream = UnixStream::connect(&socket_path)
        .map_err(|e| format!("cannot connect to daemon — is it running? ({})", e))?;

    let reader_stream = stream.try_clone()?;
    let mut writer_stream = stream;

    // Print banner
    print_banner(&term);

    // Shared state
    let waiting = Arc::new(AtomicBool::new(false));
    let waiting_reader = Arc::clone(&waiting);
    let running = Arc::new(AtomicBool::new(true));
    let running_ctrlc = Arc::clone(&running);

    // Ctrl+C
    let _ = ctrlc::set_handler(move || {
        running_ctrlc.store(false, Ordering::Relaxed);
        // Restore terminal on exit
        let _ = terminal::disable_raw_mode();
        let _ = std::io::stdout().execute(cursor::Show);
        eprintln!();
    });

    // Response reader thread
    let running_reader = Arc::clone(&running);
    let reader_handle = std::thread::spawn(move || {
        let reader = BufReader::new(reader_stream);
        let mut in_response = false;
        for line_result in reader.lines() {
            if !running_reader.load(Ordering::Relaxed) {
                break;
            }
            match line_result {
                Ok(line) => {
                    if waiting_reader.load(Ordering::Relaxed) {
                        // Clear spinner line
                        eprint!("\r\x1b[2K");
                        if !in_response {
                            // Response header
                            eprintln!();
                            eprintln!(
                                "  {BOLD_CYAN}╭─{RESET} {DIM}harmonia{RESET}"
                            );
                            in_response = true;
                        }
                    }
                    waiting_reader.store(false, Ordering::Relaxed);

                    // Print response line
                    print_agent_line(&line);
                }
                Err(_) => break,
            }
        }
        if running_reader.load(Ordering::Relaxed) {
            eprintln!("\n  {RED}✗{RESET} Connection lost.");
        }
    });

    // Main input loop with raw mode
    let result = run_input_loop(&mut writer_stream, &waiting, &running, &term);

    running.store(false, Ordering::Relaxed);
    let _ = reader_handle.join();

    eprintln!();
    eprintln!("  {BOLD_CYAN}◆{RESET} Goodbye.");
    eprintln!();

    result
}

fn run_input_loop(
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    term: &Term,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = std::io::stdout();

    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        // Wait for any pending response
        if waiting.load(Ordering::Relaxed) {
            show_thinking_spinner(waiting, running);
            if !running.load(Ordering::Relaxed) {
                break;
            }
            // Close response block
            eprintln!("  {BOLD_CYAN}╰─{RESET}");
            eprintln!();
        }

        // Show prompt and read input
        let input = read_input_line(&mut stdout, running, term)?;

        if !running.load(Ordering::Relaxed) {
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Handle commands
        if trimmed.starts_with('/') {
            match handle_command(trimmed, term, &mut stdout, writer, waiting, running) {
                CommandResult::Handled => continue,
                CommandResult::Quit => break,
                CommandResult::SendToAgent(cmd) => {
                    // Send system command to daemon
                    send_to_daemon(writer, &cmd, waiting, running)?;
                    continue;
                }
                CommandResult::Chat => {} // fall through to normal chat
            }
        }

        // Print user message echo
        eprintln!();
        eprintln!("  {BOLD_GREEN}╭─{RESET} {DIM}you{RESET}");
        for line in trimmed.lines() {
            eprintln!("  {GREEN}│{RESET} {}", line);
        }
        eprintln!("  {BOLD_GREEN}╰─{RESET}");

        // Send to daemon as chat message
        send_to_daemon(writer, trimmed, waiting, running)?;
    }

    Ok(())
}

fn read_input_line(
    stdout: &mut Stdout,
    running: &Arc<AtomicBool>,
    _term: &Term,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    let mut cursor_pos: usize = 0;
    let _history_buf: Vec<String> = Vec::new();

    // Reserve 3 lines for the input box (top border, input, bottom border)
    // Must use stdout (not stderr) since draw_prompt uses stdout cursor positioning
    writeln!(stdout)?;
    writeln!(stdout)?;
    write!(stdout, "")?;
    stdout.flush()?;

    // Draw prompt
    draw_prompt(stdout, &input, cursor_pos)?;

    terminal::enable_raw_mode()?;

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
                        running.store(false, Ordering::Relaxed);
                        break Ok(String::new());
                    }

                    // Ctrl+D — exit on empty line
                    KeyEvent {
                        code: KeyCode::Char('d'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if input.is_empty() {
                            running.store(false, Ordering::Relaxed);
                            break Ok(String::new());
                        }
                    }

                    // Ctrl+U — clear line
                    KeyEvent {
                        code: KeyCode::Char('u'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        input.clear();
                        cursor_pos = 0;
                        draw_prompt(stdout, &input, cursor_pos)?;
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
                        cursor_pos = 0;
                        draw_prompt(stdout, &input, cursor_pos)?;
                    }

                    // Ctrl+E — end of line
                    KeyEvent {
                        code: KeyCode::Char('e'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::End,
                        ..
                    } => {
                        cursor_pos = input.len();
                        draw_prompt(stdout, &input, cursor_pos)?;
                    }

                    // Ctrl+W — delete word backward
                    KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            let mut new_pos = cursor_pos;
                            // Skip trailing spaces
                            while new_pos > 0
                                && input.as_bytes().get(new_pos - 1) == Some(&b' ')
                            {
                                new_pos -= 1;
                            }
                            // Skip word chars
                            while new_pos > 0
                                && input.as_bytes().get(new_pos - 1) != Some(&b' ')
                            {
                                new_pos -= 1;
                            }
                            input.drain(new_pos..cursor_pos);
                            cursor_pos = new_pos;
                            draw_prompt(stdout, &input, cursor_pos)?;
                        }
                    }

                    // Enter — submit
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        // Clear the 3-line input box
                        // Move to top of box, clear all 3 lines
                        write!(stdout, "\x1b[1B")?; // move to bottom border line
                        write!(stdout, "\x1b[2K")?; // clear bottom border
                        write!(stdout, "\x1b[1F\x1b[2K")?; // up + clear input line
                        write!(stdout, "\x1b[1F\x1b[2K")?; // up + clear top border
                        stdout.execute(cursor::MoveToColumn(0))?;
                        stdout.flush()?;
                        break Ok(input);
                    }

                    // Backspace
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            input.remove(cursor_pos);
                            draw_prompt(stdout, &input, cursor_pos)?;
                        }
                    }

                    // Delete
                    KeyEvent {
                        code: KeyCode::Delete,
                        ..
                    } => {
                        if cursor_pos < input.len() {
                            input.remove(cursor_pos);
                            draw_prompt(stdout, &input, cursor_pos)?;
                        }
                    }

                    // Left arrow
                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            draw_prompt(stdout, &input, cursor_pos)?;
                        }
                    }

                    // Right arrow
                    KeyEvent {
                        code: KeyCode::Right,
                        ..
                    } => {
                        if cursor_pos < input.len() {
                            cursor_pos += 1;
                            draw_prompt(stdout, &input, cursor_pos)?;
                        }
                    }

                    // Regular character
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        ..
                    } => {
                        input.insert(cursor_pos, c);
                        cursor_pos += 1;
                        draw_prompt(stdout, &input, cursor_pos)?;
                    }

                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    result
}

fn term_width() -> u16 {
    crossterm::terminal::size().map(|(w, _)| w).unwrap_or(80)
}

fn draw_prompt(
    stdout: &mut Stdout,
    input: &str,
    cursor_pos: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = term_width() as usize;
    // Inner width: total minus 2 margins minus 2 border chars
    let inner = if width > 8 { width - 6 } else { 40 };

    // Move up 3 lines to redraw the box (top border, input line, bottom border)
    // On first draw these will be empty lines — harmless
    write!(stdout, "\x1b[3F")?;

    // ── Top border ──
    let top_bar = "─".repeat(inner);
    write!(stdout, "\x1b[2K")?;
    writeln!(stdout, "  {DIM}╭{top_bar}╮{RESET}")?;

    // ── Input line ──
    // Viewport scrolling: if input is wider than inner box, scroll to keep cursor visible
    let view_content_width = if inner > 3 { inner - 2 } else { inner }; // 1 char padding each side
    let (display_text, display_cursor) = if cursor_pos > view_content_width {
        let scroll = cursor_pos - view_content_width;
        let visible: String = input.chars().skip(scroll).take(view_content_width).collect();
        (visible, view_content_width)
    } else {
        let visible: String = input.chars().take(view_content_width).collect();
        (visible, cursor_pos)
    };

    let pad_len = if view_content_width > display_text.len() {
        view_content_width - display_text.len()
    } else {
        0
    };
    let padding = " ".repeat(pad_len);

    write!(stdout, "\x1b[2K")?;
    writeln!(
        stdout,
        "  {DIM}│{RESET} {BOLD_WHITE}{display_text}{RESET}{padding} {DIM}│{RESET}"
    )?;

    // ── Bottom border ──
    let bottom_bar = "─".repeat(inner);
    write!(stdout, "\x1b[2K")?;
    write!(stdout, "  {DIM}╰{bottom_bar}╯{RESET}")?;

    // Position cursor on the input line (1 line up from current position)
    // Move up 1 line, then to the correct column
    write!(stdout, "\x1b[1F")?;
    let col = (4 + display_cursor) as u16; // 2 margin + border + space
    stdout.execute(cursor::MoveToColumn(col))?;

    stdout.flush()?;
    Ok(())
}

fn show_thinking_spinner(waiting: &Arc<AtomicBool>, running: &Arc<AtomicBool>) {
    let dots = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;
    while waiting.load(Ordering::Relaxed) && running.load(Ordering::Relaxed) {
        eprint!(
            "\r  {CYAN}{}{RESET} {DIM}thinking...{RESET}  ",
            dots[i % dots.len()]
        );
        let _ = std::io::stderr().flush();
        i += 1;
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    eprint!("\r\x1b[2K");
    let _ = std::io::stderr().flush();
}

fn print_agent_line(line: &str) {
    if line.starts_with("[ERROR]") || line.starts_with("Error:") {
        eprintln!("  {RED}│{RESET} {RED}{}{RESET}", line);
    } else if line.starts_with("[WARN]") || line.starts_with("Warning:") {
        eprintln!("  {YELLOW}│{RESET} {YELLOW}{}{RESET}", line);
    } else if line.starts_with("[DEBUG]") {
        eprintln!("  {DIM}│ {}{RESET}", line);
    } else if line.starts_with("```") {
        eprintln!("  {CYAN}│{RESET} {DIM}{}{RESET}", line);
    } else if line.starts_with("# ") || line.starts_with("## ") || line.starts_with("### ") {
        eprintln!("  {CYAN}│{RESET} {BOLD_WHITE}{}{RESET}", line);
    } else if line.starts_with("- ") || line.starts_with("* ") {
        eprintln!("  {CYAN}│{RESET} {CYAN}•{RESET} {}", &line[2..]);
    } else if line.starts_with("> ") {
        eprintln!("  {CYAN}│{RESET} {DIM}▎{RESET}{DIM}{}{RESET}", &line[2..]);
    } else {
        eprintln!("  {CYAN}│{RESET} {}", line);
    }
}

fn print_banner(term: &Term) {
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

    eprintln!(
        "  {DIM}v{VERSION} — self-improving harmonic agent{RESET}"
    );
    eprintln!();
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!();
    eprintln!(
        "  {DIM}Type a message to chat with Harmonia.{RESET}"
    );
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
    waiting.store(true, Ordering::Relaxed);
    Ok(())
}

enum CommandResult {
    Handled,
    Quit,
    SendToAgent(String),
    Chat, // not a command, send as regular chat
}

fn handle_command(
    cmd: &str,
    term: &Term,
    stdout: &mut Stdout,
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> CommandResult {
    let base = cmd.split_whitespace().next().unwrap_or("");
    match base {
        // ── TUI-local commands ──
        "/help" | "/h" | "/?" => {
            print_help();
            CommandResult::Handled
        }
        "/clear" | "/cls" => {
            let _ = term.clear_screen();
            print_banner(term);
            CommandResult::Handled
        }
        "/log" | "/logs" => {
            print_recent_log();
            CommandResult::Handled
        }
        "/quit" | "/exit" | "/q" => CommandResult::Quit,

        // ── Interactive menu ──
        "/menu" | "/m" => {
            match run_menu_flow(stdout, writer, waiting, running) {
                Ok(()) => CommandResult::Handled,
                Err(_) => CommandResult::Handled,
            }
        }

        // ── System commands (sent to daemon) ──
        "/status" | "/backends" | "/frontends" | "/tools"
        | "/chronicle" | "/metrics" | "/security" | "/identity"
        | "/wallet" => {
            CommandResult::SendToAgent(cmd.to_string())
        }

        _ => CommandResult::Chat,
    }
}

fn run_menu_flow(
    stdout: &mut Stdout,
    writer: &mut UnixStream,
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut menu_stack: Vec<String> = vec!["main".to_string()];

    loop {
        if !running.load(Ordering::Relaxed) {
            break;
        }

        let current = menu_stack.last().unwrap().clone();
        let (title, items) = match current.as_str() {
            "main" => ("Harmonia", crate::menus::main_menu_items()),
            other => {
                if let Some((t, items)) = crate::menus::submenu_items(other) {
                    (t, items)
                } else {
                    break;
                }
            }
        };

        match crate::menus::interactive_select(stdout, title, &items)? {
            crate::menus::MenuAction::Command(cmd) => {
                // Send command to daemon
                eprintln!();
                eprintln!(
                    "  {DIM}→ {}{RESET}",
                    cmd
                );
                send_to_daemon(writer, &cmd, waiting, running)?;

                // Wait for response
                show_thinking_spinner(waiting, running);
                if waiting.load(Ordering::Relaxed) {
                    // Still waiting, give it a moment
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                // Close response block
                eprintln!("  {BOLD_CYAN}╰─{RESET}");
                eprintln!();

                // Stay in menu for another selection
            }
            crate::menus::MenuAction::SubMenu(name) => {
                menu_stack.push(name);
            }
            crate::menus::MenuAction::Back => {
                if menu_stack.len() > 1 {
                    menu_stack.pop();
                } else {
                    break;
                }
            }
            crate::menus::MenuAction::Cancel => {
                break;
            }
        }
    }

    Ok(())
}

fn print_help() {
    eprintln!();
    eprintln!("  {BOLD_CYAN}◆{RESET} {BOLD}Commands{RESET}");
    eprintln!("  {DIM}──────────────────────────────────────{RESET}");
    eprintln!();
    eprintln!("  {DIM}TUI{RESET}");
    eprintln!("  {CYAN}/menu{RESET}                Interactive menu");
    eprintln!("  {CYAN}/help{RESET}                Show this help");
    eprintln!("  {CYAN}/clear{RESET}               Clear screen");
    eprintln!("  {CYAN}/log{RESET}                 Recent log entries");
    eprintln!("  {CYAN}/exit{RESET}                Exit");
    eprintln!();
    eprintln!("  {DIM}System (works from any frontend){RESET}");
    eprintln!("  {CYAN}/status{RESET}              System health & info");
    eprintln!("  {CYAN}/backends{RESET} {DIM}[name]{RESET}     LLM backend config");
    eprintln!("  {CYAN}/frontends{RESET} {DIM}[name]{RESET}    Frontend channels");
    eprintln!("  {CYAN}/tools{RESET}               Tool API status");
    eprintln!();
    eprintln!("  {DIM}Observability{RESET}");
    eprintln!("  {CYAN}/chronicle{RESET} {DIM}[sub]{RESET}     History & knowledge");
    eprintln!("  {CYAN}/metrics{RESET}             Model performance");
    eprintln!("  {CYAN}/security{RESET} {DIM}[sub]{RESET}      Security audit");
    eprintln!("  {CYAN}/identity{RESET}            Wallet & vault keys");
    eprintln!();
    eprintln!("  {DIM}──────────────────────────────────────{RESET}");
    eprintln!("  {DIM}Everything else is sent to the agent.{RESET}");
    eprintln!();
}

fn print_recent_log() {
    let log_path = match crate::paths::log_path() {
        Ok(p) => p,
        Err(_) => return,
    };

    eprintln!();
    eprintln!(
        "  {BOLD_CYAN}◆{RESET} {BOLD}Recent Logs{RESET}"
    );
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
