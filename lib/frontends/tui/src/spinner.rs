// ── Spinner: thinking indicator with concurrent input ─────────────────

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossterm::{
    cursor::{self, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
    cursor::MoveTo,
    terminal::{self, Clear, ClearType},
    ExecutableCommand,
};

use crate::input::{byte_index_for_char, char_len};
use crate::prompt::draw_prompt;
use crate::theme::*;

/// Cached state root -- set once at session start, used by spinner.
static REPL_STATUS_PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub(crate) fn init_repl_status_path(state_root: &str) {
    let _ = REPL_STATUS_PATH.set(std::path::PathBuf::from(state_root).join("repl-status.txt"));
}

pub(crate) fn read_repl_status() -> String {
    REPL_STATUS_PATH
        .get()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

pub(crate) fn draw_spinner_line(row: u16, dot: &str, has_queued: bool) {
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
        crossterm::cursor::SavePosition,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(status),
        crossterm::cursor::RestorePosition
    );
    let _ = err.flush();
}

pub(crate) fn clear_spinner_and_box(spinner_row: u16, box_row: u16, box_height: u16) {
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

/// Show thinking spinner with a persistent input box below it.
/// The input box stays visible and functional -- user can type ahead.
/// Returns Some(text) if the user submitted input during thinking, None otherwise.
/// Empty-enter (no text) is silently ignored.
pub(crate) fn show_thinking_spinner_with_input(
    waiting: &Arc<AtomicBool>,
    running: &Arc<AtomicBool>,
    reader_alive: &Arc<AtomicBool>,
) -> Option<String> {
    let dots = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let mut i = 0;
    let mut input_buf = String::new();
    let mut cursor_pos: usize = 0;
    let started = std::time::Instant::now();
    // No fixed timeout. The spinner reads repl-status.txt for live error state.
    // Only give up if the status explicitly says "error" or "unavailable",
    // or if the daemon hasn't written ANY status for a long time (truly dead).
    const STALE_THRESHOLD: std::time::Duration = std::time::Duration::from_secs(300);
    let mut last_status_change = std::time::Instant::now();
    let mut prev_status = String::new();

    let _ = terminal::enable_raw_mode();

    // Reserve space: 1 row for spinner + 3 rows for input box
    let (_, term_h) = terminal::size().unwrap_or((80, 24));
    let (_, cur_row) = cursor::position().unwrap_or((0, 0));
    let needed: u16 = 4;
    let spinner_row = if cur_row + needed >= term_h {
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
        // Break if reader thread has died
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

        // Early break: if REPL reports all models unavailable
        {
            let status = read_repl_status();
            if status.contains("all models unavailable") {
                clear_spinner_and_box(spinner_row, box_row, box_height);
                let _ = terminal::disable_raw_mode();
                waiting.store(false, Ordering::Release);
                eprintln!(
                    "\n  {RED}✗{RESET} All models unavailable — check network/providers."
                );
                eprintln!();
                return if !input_buf.trim().is_empty() {
                    Some(input_buf)
                } else {
                    None
                };
            }
        }
        // Dynamic error detection: check repl-status for unrecoverable errors.
        {
            let current_status = read_repl_status();
            if current_status != prev_status {
                last_status_change = std::time::Instant::now();
                prev_status = current_status.clone();
            }
            let status_lower = current_status.to_lowercase();
            // Detect specific unrecoverable errors in repl-status.
            let is_fatal = status_lower.contains("dispatch timeout")
                || status_lower.contains("ipc failed")
                || status_lower.contains("lock poisoned")
                || status_lower.contains("panic")
                || status_lower.contains("unrecoverable");
            if is_fatal {
                clear_spinner_and_box(spinner_row, box_row, box_height);
                let _ = terminal::disable_raw_mode();
                waiting.store(false, Ordering::Release);
                eprintln!(
                    "\n  {RED}!{RESET} {current_status}",
                );
                eprintln!();
                return if !input_buf.trim().is_empty() {
                    Some(input_buf)
                } else {
                    None
                };
            }
            // Stale detection: if repl-status hasn't changed for a long time,
            // the daemon is truly unresponsive (not just slow LLM).
            if last_status_change.elapsed() > STALE_THRESHOLD {
                clear_spinner_and_box(spinner_row, box_row, box_height);
                let _ = terminal::disable_raw_mode();
                waiting.store(false, Ordering::Release);
                let _elapsed = started.elapsed().as_secs();
                eprintln!(
                    "\n  {YELLOW}!{RESET} No status update for {}s — daemon may be unresponsive. Last: {}",
                    STALE_THRESHOLD.as_secs(),
                    if current_status.is_empty() { "(none)" } else { current_status.trim() },
                );
                eprintln!();
                return if !input_buf.trim().is_empty() {
                    Some(input_buf)
                } else {
                    None
                };
            }
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
                            clear_spinner_and_box(spinner_row, box_row, box_height);
                            let _ = terminal::disable_raw_mode();
                            return Some(input_buf);
                        }
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

    // Thinking finished -- clear spinner line, leave box area clean
    clear_spinner_and_box(spinner_row, box_row, box_height);
    let _ = terminal::disable_raw_mode();

    if !input_buf.trim().is_empty() {
        Some(input_buf)
    } else {
        None
    }
}
