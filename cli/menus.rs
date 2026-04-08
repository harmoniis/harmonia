//! Interactive menu framework for Harmonia TUI sessions.
//!
//! Provides arrow-key navigable selection menus that convert
//! user choices into /commands sent through the daemon socket.

use crossterm::{
    cursor::{self, Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    queue,
    style::Print,
    terminal::{self, Clear, ClearType},
};
use std::io::{Stdout, Write};

const BOLD_CYAN: &str = "\x1b[1;36m";
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// ── Interactive select ────────────────────────────────────────────────

pub struct MenuItem {
    pub label: String,
    pub command: String, // the /command to send
    pub hint: String,    // short description
}

impl MenuItem {
    pub fn new(label: &str, command: &str, hint: &str) -> Self {
        Self {
            label: label.to_string(),
            command: command.to_string(),
            hint: hint.to_string(),
        }
    }
}

/// Result of a menu interaction
pub enum MenuAction {
    /// User selected an item — send this command to daemon
    Command(String),
    /// User wants a submenu
    SubMenu(String),
    /// User cancelled (Esc)
    Cancel,
    /// User wants to go back
    Back,
}

/// Show an interactive selection menu. Returns the selected command string or None.
pub fn interactive_select(
    _stdout: &mut Stdout,
    title: &str,
    items: &[MenuItem],
) -> Result<MenuAction, Box<dyn std::error::Error>> {
    if items.is_empty() {
        return Ok(MenuAction::Cancel);
    }

    terminal::enable_raw_mode()?;
    let result = interactive_select_inner(title, items);
    let _ = terminal::disable_raw_mode();

    match result {
        Ok((action, menu_row, total_lines)) => {
            clear_menu(menu_row, total_lines)?;
            Ok(action)
        }
        Err(e) => Err(e),
    }
}

/// Inner menu loop — runs entirely in raw mode. Caller guarantees cleanup.
fn interactive_select_inner(
    title: &str,
    items: &[MenuItem],
) -> Result<(MenuAction, u16, u16), Box<dyn std::error::Error>> {
    let mut selected: usize = 0;
    let max_label = items.iter().map(|i| i.label.len()).max().unwrap_or(0);

    // Drain any stale key events (e.g. Enter release from previous menu)
    while event::poll(std::time::Duration::from_millis(50))? {
        let _ = event::read()?;
    }

    // Flush and get cursor position for absolute positioning
    std::io::stderr().flush()?;
    std::io::stdout().flush()?;
    let (_, start_row) = cursor::position()?;
    let (_, term_h) = terminal::size()?;

    // total rows: title + separator + items + nav hint = items.len() + 3
    let total_lines = items.len() as u16 + 3;
    let max_menu_row = term_h.saturating_sub(total_lines + 1);
    let menu_row = if start_row > max_menu_row {
        let deficit = start_row - max_menu_row;
        let mut err = std::io::stderr();
        for _ in 0..deficit {
            let _ = write!(err, "\n");
        }
        let _ = err.flush();
        queue!(err, MoveTo(0, max_menu_row))?;
        err.flush()?;
        max_menu_row
    } else {
        start_row
    };

    draw_menu(title, items, selected, max_label, menu_row)?;

    let result = loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    KeyEvent {
                        code: KeyCode::Up, ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('k'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        selected = if selected > 0 {
                            selected - 1
                        } else {
                            items.len() - 1
                        };
                        draw_menu(title, items, selected, max_label, menu_row)?;
                    }

                    KeyEvent {
                        code: KeyCode::Down,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('j'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        selected = (selected + 1) % items.len();
                        draw_menu(title, items, selected, max_label, menu_row)?;
                    }

                    KeyEvent {
                        code: KeyCode::Enter,
                        ..
                    } => {
                        let item = &items[selected];
                        break if item.command.starts_with("submenu:") {
                            MenuAction::SubMenu(item.command[8..].to_string())
                        } else {
                            MenuAction::Command(item.command.clone())
                        };
                    }

                    KeyEvent {
                        code: KeyCode::Esc, ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('q'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        break MenuAction::Cancel;
                    }

                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        break MenuAction::Cancel;
                    }

                    KeyEvent {
                        code: KeyCode::Left,
                        ..
                    }
                    | KeyEvent {
                        code: KeyCode::Backspace,
                        ..
                    } => {
                        break MenuAction::Back;
                    }

                    KeyEvent {
                        code: KeyCode::Char(c @ '1'..='9'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        let idx = (c as usize) - ('1' as usize);
                        if idx < items.len() {
                            let item = &items[idx];
                            break if item.command.starts_with("submenu:") {
                                MenuAction::SubMenu(item.command[8..].to_string())
                            } else {
                                MenuAction::Command(item.command.clone())
                            };
                        }
                    }

                    _ => {}
                }
            }
        }
    };

    Ok((result, menu_row, total_lines))
}

/// Draw the menu at fixed absolute rows starting from `menu_row` (0-based).
fn draw_menu(
    title: &str,
    items: &[MenuItem],
    selected: usize,
    max_label: usize,
    menu_row: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut err = std::io::stderr();
    queue!(err, Hide)?;

    let mut row = menu_row;

    // Title
    queue!(
        err,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(format!("  {BOLD_CYAN}◆{RESET} {BOLD}{title}{RESET}"))
    )?;
    row += 1;

    // Separator
    queue!(
        err,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(format!(
            "  {DIM}──────────────────────────────────────{RESET}"
        ))
    )?;
    row += 1;

    // Items
    for (i, item) in items.iter().enumerate() {
        let num = i + 1;
        queue!(err, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
        if i == selected {
            queue!(err, Print(format!(
                "  {BOLD_CYAN}❯{RESET} {BOLD}{num}.{RESET} {BOLD_CYAN}{:<width$}{RESET}  {DIM}{}{RESET}",
                item.label, item.hint, width = max_label
            )))?;
        } else {
            queue!(
                err,
                Print(format!(
                    "    {DIM}{num}.{RESET} {:<width$}  {DIM}{}{RESET}",
                    item.label,
                    item.hint,
                    width = max_label
                ))
            )?;
        }
        row += 1;
    }

    // Navigation hint
    queue!(
        err,
        MoveTo(0, row),
        Clear(ClearType::CurrentLine),
        Print(format!(
            "  {DIM}↑↓ navigate  Enter select  Left/Backspace back  Esc close  1-9 jump{RESET}"
        ))
    )?;

    queue!(err, Show)?;
    err.flush()?;
    Ok(())
}

fn clear_menu(menu_row: u16, total_lines: u16) -> Result<(), Box<dyn std::error::Error>> {
    let mut err = std::io::stderr();
    for r in 0..total_lines {
        let _ = queue!(err, MoveTo(0, menu_row + r), Clear(ClearType::CurrentLine));
    }
    let _ = queue!(err, MoveTo(0, menu_row));
    err.flush()?;
    Ok(())
}

