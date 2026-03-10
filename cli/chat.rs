use crate::start;
use console::{style, Term};
use crossterm::{
    cursor::{self, Hide, MoveTo, MoveToColumn, RestorePosition, SavePosition, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};
use std::io::{BufRead, BufReader, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use unicode_width::UnicodeWidthChar;

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
        eprintln!("  {} Starting daemon...", style("◆").cyan().bold());
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
        let _ = terminal::disable_raw_mode();
        let _ = std::io::stderr().execute(Show);
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
                        let mut err = std::io::stderr();
                        let _ = queue!(err, MoveToColumn(0), Clear(ClearType::CurrentLine));
                        let _ = err.flush();
                        if !in_response {
                            // Response header
                            eprintln!();
                            eprintln!("  {BOLD_CYAN}╭─{RESET} {DIM}harmonia{RESET}");
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
    // Shut down socket so reader thread unblocks from lines()
    let _ = writer_stream.shutdown(std::net::Shutdown::Both);
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
        let input = read_input_line(running, term)?;

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
    running: &Arc<AtomicBool>,
    _term: &Term,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    let mut cursor_pos: usize = 0; // character index, not byte index
    let mut menu_sel: Option<usize> = None;

    terminal::enable_raw_mode()?;

    // Flush both streams so cursor::position() returns the real terminal cursor
    std::io::stderr().flush()?;
    std::io::stdout().flush()?;

    let (_, start_row) = cursor::position()?;
    let (_, term_h) = terminal::size()?;
    // Need 3 (box) + 5 (menu) = 8 rows starting from box_row
    let total_needed: u16 = 3 + SLASH_MENU_MAX as u16;
    // MoveTo is 0-based: rows box_row..box_row+total_needed-1 must fit
    // i.e. box_row + total_needed - 1 < term_h → box_row <= term_h - total_needed
    let max_box_row = term_h.saturating_sub(total_needed);
    let box_row = if start_row > max_box_row {
        let deficit = start_row - max_box_row;
        let mut err = std::io::stderr();
        for _ in 0..deficit {
            let _ = write!(err, "\n");
        }
        let _ = err.flush();
        // Move cursor up to max_box_row (0-based)
        queue!(err, MoveTo(0, max_box_row))?;
        err.flush()?;
        max_box_row
    } else {
        start_row
    };

    draw_prompt(&input, cursor_pos, box_row)?;

    // Helper: update slash menu after input changes
    let update_menu = |input: &str, menu_sel: &mut Option<usize>, box_row: u16| {
        if input.starts_with('/') {
            let m = slash_matches(input);
            if !m.is_empty() {
                let sel = menu_sel.unwrap_or(0).min(m.len() - 1);
                *menu_sel = Some(sel);
                draw_slash_menu(box_row, &m, sel);
            } else {
                *menu_sel = None;
                clear_slash_menu(box_row);
            }
        } else {
            *menu_sel = None;
            clear_slash_menu(box_row);
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
                        clear_slash_menu(box_row);
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
                            clear_slash_menu(box_row);
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
                        draw_prompt(&input, cursor_pos, box_row)?;
                        update_menu(&input, &mut menu_sel, box_row);
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
                        draw_prompt(&input, cursor_pos, box_row)?;
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
                        cursor_pos = char_len(&input);
                        draw_prompt(&input, cursor_pos, box_row)?;
                    }

                    // Ctrl+W — delete word backward
                    KeyEvent {
                        code: KeyCode::Char('w'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            let mut new_pos = cursor_pos;
                            while new_pos > 0
                                && char_at(&input, new_pos - 1)
                                    .map(|ch| ch.is_whitespace())
                                    .unwrap_or(false)
                            {
                                new_pos -= 1;
                            }
                            while new_pos > 0
                                && !char_at(&input, new_pos - 1)
                                    .map(|ch| ch.is_whitespace())
                                    .unwrap_or(true)
                            {
                                new_pos -= 1;
                            }
                            let start = byte_index_for_char(&input, new_pos);
                            let end = byte_index_for_char(&input, cursor_pos);
                            input.drain(start..end);
                            cursor_pos = new_pos;
                            draw_prompt(&input, cursor_pos, box_row)?;
                            update_menu(&input, &mut menu_sel, box_row);
                        }
                    }

                    // Up arrow — navigate slash menu
                    KeyEvent {
                        code: KeyCode::Up, ..
                    } => {
                        if let Some(sel) = menu_sel.as_mut() {
                            let m = slash_matches(&input);
                            if !m.is_empty() {
                                *sel = if *sel == 0 {
                                    m.len().min(SLASH_MENU_MAX) - 1
                                } else {
                                    *sel - 1
                                };
                                draw_slash_menu(box_row, &m, *sel);
                            }
                        }
                    }

                    // Down arrow — navigate slash menu
                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    } => {
                        if let Some(sel) = menu_sel.as_mut() {
                            let m = slash_matches(&input);
                            if !m.is_empty() {
                                let max = m.len().min(SLASH_MENU_MAX) - 1;
                                *sel = if *sel >= max { 0 } else { *sel + 1 };
                                draw_slash_menu(box_row, &m, *sel);
                            }
                        }
                    }

                    // Enter — submit (or select from slash menu)
                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        // If slash menu is open, select the highlighted command
                        if let Some(sel) = menu_sel {
                            let m = slash_matches(&input);
                            if sel < m.len() {
                                input = m[sel].0.to_string();
                            }
                        }
                        // Clear box + menu
                        let mut err = std::io::stderr();
                        let total_rows = 3 + SLASH_MENU_MAX as u16;
                        for r in 0..total_rows {
                            let _ =
                                queue!(err, MoveTo(0, box_row + r), Clear(ClearType::CurrentLine));
                        }
                        let _ = queue!(err, MoveTo(0, box_row));
                        let _ = err.flush();
                        break Ok(input);
                    }

                    // Tab — accept selected slash command into input
                    KeyEvent {
                        code: KeyCode::Tab, ..
                    } => {
                        if let Some(sel) = menu_sel {
                            let m = slash_matches(&input);
                            if sel < m.len() {
                                input = m[sel].0.to_string();
                                cursor_pos = char_len(&input);
                                draw_prompt(&input, cursor_pos, box_row)?;
                                update_menu(&input, &mut menu_sel, box_row);
                            }
                        }
                    }

                    // Escape — close slash menu
                    KeyEvent {
                        code: KeyCode::Esc, ..
                    } => {
                        if menu_sel.is_some() {
                            menu_sel = None;
                            clear_slash_menu(box_row);
                        }
                    }

                    // Backspace
                    KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            let start = byte_index_for_char(&input, cursor_pos - 1);
                            let end = byte_index_for_char(&input, cursor_pos);
                            input.drain(start..end);
                            cursor_pos -= 1;
                            draw_prompt(&input, cursor_pos, box_row)?;
                            update_menu(&input, &mut menu_sel, box_row);
                        }
                    }

                    // Delete
                    KeyEvent {
                        code: KeyCode::Delete,
                        ..
                    } => {
                        if cursor_pos < char_len(&input) {
                            let start = byte_index_for_char(&input, cursor_pos);
                            let end = byte_index_for_char(&input, cursor_pos + 1);
                            input.drain(start..end);
                            draw_prompt(&input, cursor_pos, box_row)?;
                            update_menu(&input, &mut menu_sel, box_row);
                        }
                    }

                    // Left arrow
                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    } => {
                        if cursor_pos > 0 {
                            cursor_pos -= 1;
                            draw_prompt(&input, cursor_pos, box_row)?;
                        }
                    }

                    // Right arrow
                    KeyEvent {
                        code: KeyCode::Right,
                        ..
                    } => {
                        if cursor_pos < char_len(&input) {
                            cursor_pos += 1;
                            draw_prompt(&input, cursor_pos, box_row)?;
                        }
                    }

                    // Regular character
                    KeyEvent {
                        code: KeyCode::Char(c),
                        modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                        ..
                    } => {
                        let byte_index = byte_index_for_char(&input, cursor_pos);
                        input.insert(byte_index, c);
                        cursor_pos += 1;
                        draw_prompt(&input, cursor_pos, box_row)?;
                        update_menu(&input, &mut menu_sel, box_row);
                    }

                    _ => {}
                }
            }
        }
    };

    terminal::disable_raw_mode()?;
    result
}

const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show help"),
    ("/exit", "Quit"),
    ("/menu", "Interactive menu"),
    ("/clear", "Clear screen"),
    ("/log", "Recent logs"),
    ("/status", "System health"),
    ("/backends", "LLM backends"),
    ("/frontends", "Frontend channels"),
    ("/tools", "Registered tools"),
    ("/chronicle", "Chronicle query"),
    ("/metrics", "Runtime metrics"),
    ("/security", "Security posture"),
    ("/identity", "Agent identity"),
    ("/feedback", "Style feedback"),
];

const SLASH_MENU_MAX: usize = 5;

fn slash_matches(partial: &str) -> Vec<(&'static str, &'static str)> {
    SLASH_COMMANDS
        .iter()
        .filter(|(cmd, _)| cmd.starts_with(partial))
        .copied()
        .collect()
}

/// Draw the slash command dropdown menu below the input box.
/// Uses crossterm SavePosition/RestorePosition so the cursor returns
/// to the input line after drawing.
fn draw_slash_menu(box_row: u16, matches: &[(&str, &str)], selected: usize) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    let visible = matches.len().min(SLASH_MENU_MAX);
    // Menu starts at box_row+3 (0-based: box is rows box_row, +1, +2)
    for i in 0..SLASH_MENU_MAX {
        let row = box_row + 3 + i as u16;
        let _ = queue!(err, MoveTo(0, row), Clear(ClearType::CurrentLine));
        if i < visible {
            let (cmd, desc) = matches[i];
            if i == selected {
                let _ = queue!(
                    err,
                    Print(format!("  {BOLD_CYAN}  {cmd}{RESET}  {DIM}{desc}{RESET}"))
                );
            } else {
                let _ = queue!(err, Print(format!("  {DIM}  {cmd}  {desc}{RESET}")));
            }
        }
    }
    let _ = queue!(err, RestorePosition);
    let _ = err.flush();
}

fn clear_slash_menu(box_row: u16) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    for i in 0..SLASH_MENU_MAX {
        let _ = queue!(
            err,
            MoveTo(0, box_row + 3 + i as u16),
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

fn char_at(input: &str, char_index: usize) -> Option<char> {
    input.chars().nth(char_index)
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

fn viewport_for_input(input: &str, cursor_pos: usize, view_width: usize) -> (String, usize) {
    if view_width == 0 {
        return (String::new(), 0);
    }

    let chars: Vec<char> = input.chars().collect();
    let widths: Vec<usize> = chars
        .iter()
        .map(|ch| UnicodeWidthChar::width(*ch).unwrap_or(0))
        .collect();

    let cursor_pos = cursor_pos.min(chars.len());
    let mut start = 0usize;
    let mut cursor_col = widths[..cursor_pos].iter().sum::<usize>();
    while cursor_col > view_width && start < cursor_pos {
        cursor_col = cursor_col.saturating_sub(widths[start]);
        start += 1;
    }

    let mut end = start;
    let mut used = 0usize;
    while end < chars.len() {
        let width = widths[end];
        if used + width > view_width {
            break;
        }
        used += width;
        end += 1;
    }

    let display_text: String = chars[start..end].iter().collect();
    let display_cursor = widths[start..cursor_pos].iter().sum::<usize>();
    (display_text, display_cursor.min(view_width))
}

/// Draw the input box and position the cursor on the text line.
///
/// All coordinates use crossterm's MoveTo which is 0-based.
/// Layout (0-based rows from box_row):
///   box_row+0: ╭───────╮  (top border)
///   box_row+1: │ text   │  (input — cursor goes here)
///   box_row+2: ╰───────╯  (bottom border)
///
/// Column layout (0-based):
///   0-1: margin spaces
///   2:   │ border
///   3:   space
///   4+:  text starts → cursor col = 4 + display_cursor
fn draw_prompt(
    input: &str,
    cursor_pos: usize,
    box_row: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut err = std::io::stderr();
    let width = term_width() as usize;
    let inner = if width > 8 { width - 6 } else { 40 };
    let view_w = if inner > 2 { inner - 2 } else { inner };

    let (display_text, display_cursor) = viewport_for_input(input, cursor_pos, view_w);
    let pad = " ".repeat(view_w.saturating_sub(display_width(&display_text)));
    let bar = "─".repeat(inner);

    // Hide cursor during redraw to prevent flicker
    queue!(err, Hide)?;

    // Top border (row box_row)
    queue!(
        err,
        MoveTo(0, box_row),
        Clear(ClearType::CurrentLine),
        Print(format!("  {DIM}╭{bar}╮{RESET}"))
    )?;

    // Input line (row box_row+1)
    queue!(
        err,
        MoveTo(0, box_row + 1),
        Clear(ClearType::CurrentLine),
        Print(format!(
            "  {DIM}│{RESET} {BOLD_WHITE}{display_text}{RESET}{pad} {DIM}│{RESET}"
        ))
    )?;

    // Bottom border (row box_row+2)
    queue!(
        err,
        MoveTo(0, box_row + 2),
        Clear(ClearType::CurrentLine),
        Print(format!("  {DIM}╰{bar}╯{RESET}"))
    )?;

    // Show cursor and place it on the input line at the correct column
    queue!(err, Show, MoveTo(4 + display_cursor as u16, box_row + 1))?;

    err.flush()?;
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
    let mut err = std::io::stderr();
    let _ = queue!(err, MoveToColumn(0), Clear(ClearType::CurrentLine));
    let _ = err.flush();
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

    eprintln!("  {DIM}v{VERSION} — self-improving harmonic agent{RESET}");
    eprintln!();
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!();
    eprintln!("  {DIM}Type a message to chat with Harmonia.{RESET}");
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
    stdout: &mut std::io::Stdout,
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
        "/menu" | "/m" => match run_menu_flow(stdout, writer, waiting, running) {
            Ok(()) => CommandResult::Handled,
            Err(_) => CommandResult::Handled,
        },

        // ── System commands (sent to daemon) ──
        "/status" | "/backends" | "/frontends" | "/tools" | "/chronicle" | "/metrics"
        | "/security" | "/identity" | "/feedback" | "/wallet" => {
            CommandResult::SendToAgent(cmd.to_string())
        }

        _ => CommandResult::Chat,
    }
}

fn run_menu_flow(
    stdout: &mut std::io::Stdout,
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
                eprintln!("  {DIM}→ {}{RESET}", cmd);
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
    eprintln!("  {CYAN}/feedback{RESET} {DIM}<note>{RESET}    Record style feedback");
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
