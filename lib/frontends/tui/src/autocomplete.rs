// ── Autocomplete: slash commands and @file completion ─────────────────

use std::io::Write;
use std::path::Path;

use crossterm::{
    cursor::{RestorePosition, SavePosition},
    queue,
    style::Print,
    cursor::MoveTo,
    terminal::{self, Clear, ClearType},
};

use crate::theme::*;

pub(crate) const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show this help"),
    ("/exit", "Exit session"),
    ("/clear", "New session, clear screen"),
    ("/resume", "Resume a past session"),
    ("/rewind", "Rewind to a previous turn"),
    ("/status", "System health + subsystems"),
    ("/providers", "Active providers by category"),
    ("/tools", "Registered tools"),
    ("/session", "Current session info"),
    ("/frontends", "Setup and pair frontends"),
    ("/log", "Recent log entries"),
    ("/menu", "Interactive menu"),
    ("/chronicle", "Chronicle event query"),
    ("/metrics", "Runtime metrics"),
    ("/security", "Security posture"),
    ("/feedback", "Response style feedback"),
    ("/identity", "Agent identity"),
    ("/wallet", "Wallet/vault status"),
    ("/policies", "Channel sender policies"),
];

pub(crate) const SLASH_MENU_MAX: usize = 8;

pub(crate) fn slash_matches(partial: &str) -> Vec<(&'static str, &'static str)> {
    let query = partial.to_lowercase();
    SLASH_COMMANDS
        .iter()
        .filter(|(cmd, desc)| {
            cmd.starts_with(partial)
                || cmd[1..].contains(&query[1..].to_string())
                || desc.to_lowercase().contains(&query[1..])
        })
        .copied()
        .collect()
}

/// Draw the command palette below the input box.
pub(crate) fn draw_slash_menu(
    box_row: u16,
    box_height: u16,
    matches: &[(&str, &str)],
    selected: usize,
) {
    let mut err = std::io::stderr();
    let _ = queue!(err, SavePosition);
    let (term_w, _) = terminal::size().unwrap_or((80, 24));
    let visible = matches.len().min(SLASH_MENU_MAX);
    for i in 0..SLASH_MENU_MAX {
        let row = box_row + box_height + i as u16;
        let _ = queue!(err, MoveTo(0, row), Clear(ClearType::CurrentLine));
        if i < visible {
            let (cmd, desc) = matches[i];
            let cmd_width = cmd.len() + 4;
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
pub(crate) fn draw_file_menu(
    box_row: u16,
    box_height: u16,
    matches: &[FileMatch],
    selected: usize,
) {
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

pub(crate) fn clear_menu(box_row: u16, box_height: u16) {
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

// ── Autocomplete types ───────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct FileMatch {
    pub display: String,
    pub full_path: String,
    pub is_dir: bool,
}

pub(crate) enum AutocompleteMode {
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
pub(crate) fn find_at_token(input: &str, cursor_pos: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = input.chars().collect();
    let pos = cursor_pos.min(chars.len());
    let mut i = pos;
    while i > 0 {
        i -= 1;
        if chars[i] == '@' {
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
pub(crate) fn file_matches(workspace: &Path, partial: &str) -> Vec<FileMatch> {
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
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if !prefix.is_empty() && !name.to_lowercase().starts_with(&prefix_lower) {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        let full_path = format!("{}{}{}", parent_dir, name, if is_dir { "/" } else { "" });
        let display = full_path.clone();
        let fm = FileMatch {
            display,
            full_path,
            is_dir,
        };
        if is_dir {
            dirs.push(fm);
        } else {
            files.push(fm);
        }
    }
    dirs.append(&mut files);
    dirs.truncate(SLASH_MENU_MAX);
    dirs
}
