// ── Session flows: resume, rewind, replay, help, log, session info ────
//
// CLI-specific interactive flows. Session data operations delegate to the
// gateway session service (harmonia_gateway::sessions). The TUI library
// handles all terminal rendering; these functions orchestrate the CLI-side
// data and user interaction.

use std::path::Path;

use console::Term;
use harmonia_gateway::sessions as gsess;

const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const BOLD_CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[32m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// ── Resume flow ──────────────────────────────────────────────────────

pub(crate) fn run_resume(
    stdout: &mut std::io::Stdout,
    current_session: &gsess::Session,
    node_label: &str,
    data_dir: &Path,
) -> Result<bool, Box<dyn std::error::Error>> {
    let sessions = gsess::list(node_label, data_dir)
        .map_err(|e| format!("list sessions: {e}"))?;
    if sessions.is_empty() {
        eprintln!("\n  {DIM}No past sessions found.{RESET}\n");
        return Ok(false);
    }

    let items: Vec<crate::menus::MenuItem> = sessions.iter().map(|s| {
        let ts = gsess::format_timestamp_ms(s.updated_at_ms);
        let created = gsess::format_timestamp_ms(s.created_at_ms);
        let tag = if s.id == current_session.id { " (current)" } else { "" };
        crate::menus::MenuItem::new(
            &format!("{}{}", ts, tag), &s.id,
            &format!("{} events, created {}", s.event_count, created),
        )
    }).collect();

    match crate::menus::interactive_select(stdout, "Resume Session", &items)? {
        crate::menus::MenuAction::Command(id) => {
            let resumed = gsess::resume(node_label, data_dir, &id)
                .map_err(|e| format!("resume: {e}"))?;
            eprintln!();
            replay_session_history(&resumed, node_label);
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Rewind flow ──────────────────────────────────────────────────────

pub(crate) fn run_rewind(
    stdout: &mut std::io::Stdout,
    session: &gsess::Session,
    term: &Term,
) -> Result<bool, Box<dyn std::error::Error>> {
    let events = gsess::read_events(session).map_err(|e| format!("read events: {e}"))?;
    if events.is_empty() {
        eprintln!("\n  {DIM}No conversation to rewind.{RESET}\n");
        return Ok(false);
    }

    let raw_lines: Vec<String> = std::fs::read_to_string(&session.events_path)?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.to_string())
        .collect();

    let turns = parse_turns(&raw_lines);
    if turns.is_empty() {
        eprintln!("\n  {DIM}No conversation turns to rewind to.{RESET}\n");
        return Ok(false);
    }

    let items: Vec<crate::menus::MenuItem> = turns.iter().rev().map(|t| {
        let clipped = if t.user_text.len() > 50 {
            format!("{}...", &t.user_text[..50])
        } else { t.user_text.clone() };
        crate::menus::MenuItem::new(
            &format!("Turn {} \u{2014} {}", t.index, clipped),
            &t.index.to_string(), &t.assistant_preview,
        )
    }).collect();

    match crate::menus::interactive_select(stdout, "Rewind to turn", &items)? {
        crate::menus::MenuAction::Command(selected) => {
            let turn_num: usize = selected.parse().unwrap_or(0);
            let turn = match turns.iter().find(|t| t.index == turn_num) {
                Some(t) => t,
                None => { eprintln!("\n  {DIM}Turn not found.{RESET}\n"); return Ok(false); }
            };
            let kept: Vec<&str> = raw_lines[..turn.event_end]
                .iter()
                .map(|s| s.as_str())
                .collect();
            std::fs::write(&session.events_path, kept.join("\n") + "\n")?;

            let _ = term.clear_screen();
            let label = &session.node_label;
            harmonia_tui::render::print_banner(term, label, &session.id);
            replay_session_history(session, label);

            let removed = turns.len() - turn_num;
            eprintln!("  {BOLD_CYAN}\u{25c6}{RESET} Rewound to turn {}. {} turn{} removed.",
                turn_num, removed, if removed == 1 { "" } else { "s" });
            eprintln!();
            Ok(true)
        }
        _ => Ok(false),
    }
}

// ── Turn parsing ─────────────────────────────────────────────────────

struct Turn { index: usize, user_text: String, assistant_preview: String, event_end: usize }

fn parse_turns(events: &[String]) -> Vec<Turn> {
    let mut turns: Vec<Turn> = Vec::new();
    let mut i = 0;
    while i < events.len() {
        if let Ok(ev) = serde_json::from_str::<serde_json::Value>(&events[i]) {
            let actor = ev.get("actor").and_then(|v| v.as_str()).unwrap_or("");
            let kind = ev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let text = ev.get("text").and_then(|v| v.as_str()).unwrap_or("");

            if actor == "you" && kind == "user" && !text.starts_with('/') {
                let (lines, j) = collect_assistant_lines(events, i + 1);
                let preview = if lines.is_empty() { "(no response)".to_string() }
                    else if lines[0].len() > 60 { format!("{}...", &lines[0][..60]) }
                    else { lines[0].clone() };
                turns.push(Turn {
                    index: turns.len() + 1, user_text: text.to_string(),
                    assistant_preview: preview, event_end: j,
                });
                i = j;
                continue;
            }
        }
        i += 1;
    }
    turns
}

fn collect_assistant_lines(events: &[String], start: usize) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut j = start;
    while j < events.len() {
        if let Ok(rev) = serde_json::from_str::<serde_json::Value>(&events[j]) {
            let ra = rev.get("actor").and_then(|v| v.as_str()).unwrap_or("");
            let rk = rev.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let rt = rev.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if ra == "harmonia" && rk == "assistant" { lines.push(rt.to_string()); j += 1; continue; }
        }
        break;
    }
    (lines, j)
}

// ── Replay session history ───────────────────────────────────────────

fn replay_session_history(session: &gsess::Session, node_label: &str) {
    let events = match gsess::read_events(session) {
        Ok(e) if !e.is_empty() => e,
        _ => return,
    };

    let start = events.iter()
        .rposition(|e| e.kind == "session-open")
        .map(|i| i + 1).unwrap_or(0);
    let replay = &events[start..];
    if replay.is_empty() { return; }

    eprintln!("  {DIM}\u{2500}\u{2500} session history \u{2500}\u{2500}{RESET}");
    eprintln!();
    render_event_blocks(replay, node_label);
    eprintln!("  {DIM}\u{2500}\u{2500} end of history \u{2500}\u{2500}{RESET}");
    eprintln!();
}

fn render_event_blocks(events: &[gsess::SessionEvent], node_label: &str) {
    let mut i = 0;
    while i < events.len() {
        let ev = &events[i];
        match (ev.actor.as_str(), ev.kind.as_str()) {
            ("you", "user") => {
                eprintln!("  {BOLD_GREEN}\u{256d}\u{2500}{RESET} {DIM}you@{node_label}{RESET}");
                eprintln!("  {GREEN}\u{2502}{RESET} {}", ev.text);
                eprintln!("  {BOLD_GREEN}\u{2570}\u{2500}{RESET}");
                eprintln!();
            }
            ("harmonia", "assistant") => {
                eprintln!("  {BOLD_CYAN}\u{256d}\u{2500}{RESET} {DIM}harmonia@{node_label}{RESET}");
                let mut j = i;
                while j < events.len()
                    && events[j].actor == "harmonia"
                    && events[j].kind == "assistant"
                {
                    eprintln!("  {CYAN}\u{2502}{RESET} {}", events[j].text);
                    j += 1;
                }
                eprintln!("  {BOLD_CYAN}\u{2570}\u{2500}{RESET}");
                eprintln!();
                i = j; continue;
            }
            _ => {}
        }
        i += 1;
    }
}

// ── Print helpers ────────────────────────────────────────────────────

pub(crate) fn print_session_info(
    session: &gsess::Session,
    node: &crate::paths::NodeIdentity,
) {
    let bar = "\u{2500}".repeat(38);
    eprintln!();
    eprintln!("  {BOLD_CYAN}\u{25c6}{RESET} {BOLD}Session{RESET}");
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!("  {CYAN}id{RESET}          {}", session.id);
    eprintln!("  {CYAN}node{RESET}        {}", session.node_label);
    eprintln!("  {CYAN}role{RESET}        {}", node.role.as_str());
    eprintln!("  {CYAN}profile{RESET}     {}", node.install_profile.as_str());
    eprintln!("  {CYAN}path{RESET}        {}", session.events_dir.display());
    eprintln!("  {CYAN}events{RESET}      {}", session.events_path.display());
    eprintln!();
}

pub(crate) fn print_log() {
    let log_path = match crate::paths::log_path() { Ok(p) => p, Err(_) => return };
    let bar = "\u{2500}".repeat(34);
    eprintln!();
    eprintln!("  {BOLD_CYAN}\u{25c6}{RESET} {BOLD}Recent Logs{RESET}");
    eprintln!("  {DIM}{bar}{RESET}");
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(15);
            for line in &lines[start..] {
                if line.contains("[ERROR]") { eprintln!("  {RED}\u{2502}{RESET} {RED}{line}{RESET}"); }
                else if line.contains("[WARN]") { eprintln!("  {YELLOW}\u{2502}{RESET} {YELLOW}{line}{RESET}"); }
                else if line.contains("[DEBUG]") { eprintln!("  {DIM}\u{2502} {line}{RESET}"); }
                else if line.contains("[INFO]") { eprintln!("  {CYAN}\u{2502}{RESET} {line}"); }
                else { eprintln!("  {DIM}\u{2502}{RESET} {DIM}{line}{RESET}"); }
            }
        }
        Err(_) => eprintln!("  {DIM}\u{2502} No log file found.{RESET}"),
    }
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!();
}

pub(crate) fn print_help_text() {
    let bar = "\u{2500}".repeat(38);
    eprintln!();
    eprintln!("  {BOLD_CYAN}\u{25c6}{RESET} {BOLD}Commands{RESET}");
    eprintln!("  {DIM}{bar}{RESET}");
    eprintln!("  {CYAN}/help{RESET}       Show this help");
    eprintln!("  {CYAN}/exit{RESET}       Exit session");
    eprintln!("  {CYAN}/clear{RESET}      New session, clear screen");
    eprintln!("  {CYAN}/resume{RESET}     Resume a past session");
    eprintln!("  {CYAN}/rewind{RESET}     Rewind to a previous turn");
    eprintln!("  {CYAN}/status{RESET}     System health + subsystems");
    eprintln!("  {CYAN}/backends{RESET}   Active backends by category");
    eprintln!("  {CYAN}/tools{RESET}      Registered tools");
    eprintln!("  {CYAN}/session{RESET}    Current session info");
    eprintln!("  {CYAN}/frontends{RESET}  Setup and pair frontends");
    eprintln!("  {CYAN}/log{RESET}        Recent log entries");
    eprintln!("  {CYAN}/policies{RESET}   Channel sender policies");
    eprintln!();
}
