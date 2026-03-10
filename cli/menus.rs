//! Interactive menu framework for Harmonia TUI.
//!
//! Provides arrow-key navigable selection menus that convert
//! user choices into /commands sent through the daemon socket.

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    terminal,
    ExecutableCommand,
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
    stdout: &mut Stdout,
    title: &str,
    items: &[MenuItem],
) -> Result<MenuAction, Box<dyn std::error::Error>> {
    if items.is_empty() {
        return Ok(MenuAction::Cancel);
    }

    let mut selected: usize = 0;

    // Calculate max label width for alignment
    let max_label = items.iter().map(|i| i.label.len()).max().unwrap_or(0);

    terminal::enable_raw_mode()?;

    // Draw initial menu
    draw_menu(stdout, title, items, selected, max_label)?;

    let result = loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key {
                    // Navigation
                    KeyEvent {
                        code: KeyCode::Up, ..
                    }
                    | KeyEvent {
                        code: KeyCode::Char('k'),
                        modifiers: KeyModifiers::NONE,
                        ..
                    } => {
                        if selected > 0 {
                            selected -= 1;
                        } else {
                            selected = items.len() - 1;
                        }
                        draw_menu(stdout, title, items, selected, max_label)?;
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
                        draw_menu(stdout, title, items, selected, max_label)?;
                    }

                    // Select
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

                    // Cancel
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

                    // Ctrl+C
                    KeyEvent {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::CONTROL,
                        ..
                    } => {
                        break MenuAction::Cancel;
                    }

                    // Back
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

                    // Number shortcuts
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

    terminal::disable_raw_mode()?;

    // Clear menu lines
    clear_menu(stdout, items.len() + 4)?;

    Ok(result)
}

fn draw_menu(
    stdout: &mut Stdout,
    title: &str,
    items: &[MenuItem],
    selected: usize,
    max_label: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Move to start of menu area
    let total_lines = items.len() + 4; // title + separator + items + nav hint + blank
    for _ in 0..total_lines {
        write!(stdout, "\x1b[2K\r\n")?;
    }
    // Move back up
    stdout.execute(cursor::MoveUp(total_lines as u16))?;

    // Title
    write!(stdout, "\r\x1b[2K  {BOLD_CYAN}◆{RESET} {BOLD}{title}{RESET}\r\n")?;
    write!(
        stdout,
        "\r\x1b[2K  {DIM}──────────────────────────────────────{RESET}\r\n"
    )?;

    // Items
    for (i, item) in items.iter().enumerate() {
        let num = i + 1;
        if i == selected {
            write!(
                stdout,
                "\r\x1b[2K  {BOLD_CYAN}❯{RESET} {BOLD}{num}.{RESET} {BOLD_CYAN}{:<width$}{RESET}  {DIM}{}{RESET}\r\n",
                item.label,
                item.hint,
                width = max_label
            )?;
        } else {
            write!(
                stdout,
                "\r\x1b[2K    {DIM}{num}.{RESET} {:<width$}  {DIM}{}{RESET}\r\n",
                item.label,
                item.hint,
                width = max_label
            )?;
        }
    }

    // Navigation hint
    write!(
        stdout,
        "\r\x1b[2K  {DIM}↑↓ navigate  Enter select  Esc cancel  1-9 jump{RESET}\r\n"
    )?;

    stdout.flush()?;
    Ok(())
}

fn clear_menu(
    stdout: &mut Stdout,
    lines: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Move up and clear each line
    stdout.execute(cursor::MoveUp(1))?; // we're already one line past
    for _ in 0..lines {
        write!(stdout, "\r\x1b[2K")?;
        stdout.execute(cursor::MoveUp(1))?;
    }
    write!(stdout, "\r\x1b[2K")?;
    stdout.flush()?;
    Ok(())
}

// ── Menu definitions ──────────────────────────────────────────────────

pub fn main_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Status", "/status", "System status & health"),
        MenuItem::new("Backends", "submenu:backends", "LLM provider configuration"),
        MenuItem::new("Frontends", "submenu:frontends", "Communication channels"),
        MenuItem::new("Tools", "/tools", "Tool API keys & status"),
        MenuItem::new("Chronicle", "submenu:chronicle", "Observability & history"),
        MenuItem::new("Metrics", "/metrics", "Model performance data"),
        MenuItem::new("Security", "submenu:security", "Security audit & posture"),
        MenuItem::new("Identity", "/identity", "Wallet & vault keys"),
    ]
}

pub fn backends_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Overview", "/backends", "List all backends with key status"),
        MenuItem::new("OpenRouter", "/backends openrouter", "OpenRouter backend details"),
        MenuItem::new("OpenAI", "/backends openai", "OpenAI backend details"),
        MenuItem::new("Anthropic", "/backends anthropic", "Anthropic backend details"),
        MenuItem::new("xAI", "/backends xai", "xAI backend details"),
        MenuItem::new("Google AI", "/backends google-ai-studio", "Google AI Studio details"),
        MenuItem::new("Google Vertex", "/backends google-vertex", "Google Vertex details"),
        MenuItem::new("Amazon Bedrock", "/backends amazon-bedrock", "Amazon Bedrock details"),
        MenuItem::new("Groq", "/backends groq", "Groq backend details"),
        MenuItem::new("Alibaba", "/backends alibaba", "Alibaba backend details"),
    ]
}

pub fn frontends_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Overview", "/frontends", "List all frontends with status"),
        MenuItem::new("TUI", "/frontends tui", "Terminal interface status"),
        MenuItem::new("MQTT", "/frontends mqtt", "MQTT broker details"),
        MenuItem::new("Telegram", "/frontends telegram", "Telegram bot details"),
        MenuItem::new("Slack", "/frontends slack", "Slack bot details"),
        MenuItem::new("Discord", "/frontends discord", "Discord bot details"),
        MenuItem::new("WhatsApp", "/frontends whatsapp", "WhatsApp bridge details"),
        MenuItem::new("iMessage", "/frontends imessage", "iMessage bridge details"),
        MenuItem::new("Signal", "/frontends signal", "Signal bridge details"),
        MenuItem::new("Tailscale", "/frontends tailscale", "Mesh network details"),
    ]
}

pub fn chronicle_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Overview", "/chronicle", "Chronicle summary & GC status"),
        MenuItem::new("Harmony", "/chronicle harmony", "Harmonic state trajectory"),
        MenuItem::new("Delegation", "/chronicle delegation", "Model delegation report"),
        MenuItem::new("Costs", "/chronicle costs", "Cost analysis"),
        MenuItem::new("Graph", "/chronicle graph", "Concept graph overview"),
        MenuItem::new("GC Status", "/chronicle gc", "Garbage collection pressure"),
    ]
}

pub fn security_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Overview", "/security", "Security audit summary"),
        MenuItem::new("Posture", "/security posture", "Current security posture details"),
        MenuItem::new("Errors", "/security errors", "Recent errors from error ring"),
    ]
}

/// Resolve submenu name to items
pub fn submenu_items(name: &str) -> Option<(&str, Vec<MenuItem>)> {
    match name {
        "backends" => Some(("Backends", backends_menu_items())),
        "frontends" => Some(("Frontends", frontends_menu_items())),
        "chronicle" => Some(("Chronicle", chronicle_menu_items())),
        "security" => Some(("Security", security_menu_items())),
        _ => None,
    }
}
