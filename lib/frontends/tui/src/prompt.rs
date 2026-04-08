// ── Prompt: multiline input box drawing ───────────────────────────────

use std::io::Write;

use crossterm::{
    cursor::{Hide, SetCursorStyle, Show},
    queue,
    style::Print,
    cursor::MoveTo,
    terminal::{Clear, ClearType},
};
use unicode_width::UnicodeWidthChar;

use crate::input::{display_width, term_width};
use crate::theme::*;

/// Wrap input text into visual lines that fit within `view_width`.
/// Returns (lines, cursor_line, cursor_col).
pub(crate) fn wrap_input(
    input: &str,
    cursor_pos: usize,
    view_width: usize,
) -> (Vec<String>, usize, usize) {
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
pub(crate) fn draw_prompt(
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

    // Position cursor -- steady block style
    queue!(
        err,
        SetCursorStyle::SteadyBlock,
        Show,
        MoveTo(4 + cursor_col as u16, box_row + 1 + cursor_line as u16)
    )?;

    err.flush()?;
    Ok(box_height)
}
