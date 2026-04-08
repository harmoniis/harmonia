// ── Render: formatting and display functions ─────────────────────────

use std::io::Write;
use std::sync::{Arc, Mutex};

use console::Term;

use crate::bridge::try_unwrap_json_text;
use crate::input::term_width;
use crate::theme::*;

/// Drain the shared response buffer and render all lines.
/// Called from the main thread AFTER the spinner has cleaned up,
/// so there's no cursor conflict.
pub(crate) fn render_buffered_response(
    response_buf: &Arc<Mutex<Vec<String>>>,
    assistant_label: &str,
) {
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

pub(crate) fn print_agent_line(line: &str) {
    // Prefix: "  | " = 4 visible columns (2 margin + border + space)
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
pub(crate) fn print_wrapped(text: &str, first_prefix: &str, cont_prefix: &str, color: &str) {
    let tw = term_width() as usize;
    // Visible prefix: "  | " = 4 cols left, plus 4 cols right margin
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

pub fn print_banner(term: &Term, node_label: &str, session_id: &str) {
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
