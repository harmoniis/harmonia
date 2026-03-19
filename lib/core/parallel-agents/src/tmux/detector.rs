//! Terminal output state detection.
//!
//! Examines captured tmux pane output and determines what state the CLI
//! agent is in. This is the "ears" of the swarm — it listens to each
//! agent's terminal and reports its harmonic state.
//!
//! Detection is priority-ordered: error > completion > permission >
//! confirmation > selection > processing > input. Higher-priority states
//! override lower ones when multiple patterns match.

use super::cli_profiles::{profile_for, CliProfile};
use crate::model::{CliState, CliType};

/// Detect the current state of a CLI agent from its terminal output.
#[allow(dead_code)]
pub(crate) fn detect_state(output: &str, cli_type: &CliType) -> CliState {
    let profile = profile_for(cli_type);
    detect_with_profile(output, profile)
}

#[allow(dead_code)]
fn detect_with_profile(output: &str, profile: &CliProfile) -> CliState {
    let lines: Vec<&str> = output.lines().collect();
    let window_start = if lines.len() > profile.detection_window {
        lines.len() - profile.detection_window
    } else {
        0
    };
    let tail = &lines[window_start..];
    let tail_text = tail.join("\n");

    // Strip ANSI escape sequences for cleaner matching
    let clean = strip_ansi(&tail_text);

    // Priority 1: Error detection
    if let Some(error) = detect_error(&clean, profile) {
        return CliState::Error(error);
    }

    // Priority 2: Completion (CLI exited back to shell)
    if detect_completion(&clean, &lines, profile) {
        return CliState::Completed;
    }

    // Priority 3: Onboarding/survey/first-run (auto-dismiss before other interactive checks)
    if detect_onboarding(&clean, profile) {
        return CliState::Onboarding;
    }

    // Priority 4: Permission prompt
    if let Some((tool, desc)) = detect_permission(&clean, profile) {
        return CliState::WaitingForPermission {
            tool_name: tool,
            description: desc,
        };
    }

    // Priority 5: Plan mode (accept/reject)
    if detect_plan_mode(&clean, profile) {
        return CliState::PlanMode;
    }

    // Priority 6: Yes/No confirmation
    if let Some(question) = detect_confirmation(&clean, profile) {
        return CliState::WaitingForConfirmation { question };
    }

    // Priority 7: Selection menu
    if let Some(options) = detect_selection(&clean, profile) {
        return CliState::WaitingForSelection { options };
    }

    // Priority 8: Processing (thinking/working)
    if detect_processing(&clean, profile) {
        return CliState::Processing;
    }

    // Priority 9: Waiting for input
    if detect_input_prompt(&clean, profile) {
        return CliState::WaitingForInput;
    }

    // Default: still processing (no recognizable pattern)
    CliState::Processing
}

#[allow(dead_code)]
fn detect_error(text: &str, profile: &CliProfile) -> Option<String> {
    for pattern in profile.error_patterns {
        if let Some(pos) = text.find(pattern) {
            // Extract the error line, truncated to prevent partial LLM output leaking
            let rest = &text[pos..];
            let line_end = rest.find('\n').unwrap_or(rest.len());
            let line = &rest[..line_end];
            let truncated = if line.len() > 120 {
                format!("{}...", &line[..120])
            } else {
                line.to_string()
            };
            return Some(truncated);
        }
    }
    None
}

#[allow(dead_code)]
fn detect_completion(text: &str, all_lines: &[&str], profile: &CliProfile) -> bool {
    // Check if the very last non-empty line looks like a shell prompt.
    // Shell prompts typically end with "$ " or "% " (possibly preceded by
    // username, hostname, path, or other decorations).
    let last_non_empty = all_lines.iter().rev().find(|l| !l.trim().is_empty());
    if let Some(last) = last_non_empty {
        let clean_last = strip_ansi(last).trim().to_string();
        for pattern in profile.completion_patterns {
            // For shell prompt patterns ("$ ", "% "), check that the line
            // ends with the pattern (the shell prompt is the final thing).
            // For other patterns ("Session ended", etc.), substring match is fine.
            if pattern.ends_with(' ') && (*pattern == "$ " || *pattern == "% ") {
                if clean_last.ends_with(pattern.trim()) || clean_last.ends_with(pattern) {
                    return true;
                }
            } else if clean_last.contains(pattern) {
                return true;
            }
        }
    }
    // Also check the tail window for explicit exit markers
    for pattern in profile.completion_patterns {
        if text.contains(pattern) && text.contains("exited") {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn detect_permission(text: &str, profile: &CliProfile) -> Option<(String, String)> {
    let mut score = 0;
    let mut best_line = String::new();

    for pattern in profile.permission_patterns {
        if text.contains(pattern) {
            score += 1;
            if best_line.is_empty() {
                // Find the line containing this pattern
                for line in text.lines() {
                    if line.contains(pattern) {
                        best_line = line.trim().to_string();
                        break;
                    }
                }
            }
        }
    }

    // Need at least 2 permission-related patterns to be confident
    if score >= 2 {
        let tool = extract_tool_name(text);
        Some((tool, best_line))
    } else {
        None
    }
}

#[allow(dead_code)]
fn detect_confirmation(text: &str, profile: &CliProfile) -> Option<String> {
    for pattern in profile.confirmation_patterns {
        if text.contains(pattern) {
            // Find the line with the question
            for line in text.lines().rev() {
                let trimmed = line.trim();
                if trimmed.contains(pattern) || trimmed.ends_with('?') {
                    return Some(trimmed.to_string());
                }
            }
            return Some(pattern.to_string());
        }
    }
    None
}

#[allow(dead_code)]
fn detect_selection(text: &str, profile: &CliProfile) -> Option<Vec<String>> {
    let mut is_selection = false;
    for pattern in profile.selection_patterns {
        if text.contains(pattern) {
            is_selection = true;
            break;
        }
    }
    if !is_selection {
        return None;
    }

    // Try to extract option labels from lines that look like menu items
    let mut options = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // Common selection patterns: "  > Option" or "  [ ] Option" or "  1. Option"
        if trimmed.starts_with("> ")
            || trimmed.starts_with("❯ ")
            || trimmed.starts_with("[ ] ")
            || trimmed.starts_with("[x] ")
            || trimmed.starts_with("(●) ")
            || trimmed.starts_with("(○) ")
        {
            let label = trimmed
                .trim_start_matches("> ")
                .trim_start_matches("❯ ")
                .trim_start_matches("[ ] ")
                .trim_start_matches("[x] ")
                .trim_start_matches("(●) ")
                .trim_start_matches("(○) ")
                .to_string();
            if !label.is_empty() {
                options.push(label);
            }
        }
    }

    if options.is_empty() {
        None
    } else {
        Some(options)
    }
}

#[allow(dead_code)]
fn detect_processing(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.processing_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn detect_input_prompt(text: &str, profile: &CliProfile) -> bool {
    // Check the last few non-empty lines for prompt patterns
    let last_lines: Vec<&str> = text
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(3)
        .collect();

    for line in &last_lines {
        for pattern in profile.input_prompt_patterns {
            if line.contains(pattern) {
                return true;
            }
        }
    }
    false
}

#[allow(dead_code)]
fn detect_onboarding(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.onboarding_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn detect_plan_mode(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.plan_mode_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

/// Try to extract a tool name from permission prompt text.
#[allow(dead_code)]
fn extract_tool_name(text: &str) -> String {
    // Look for common tool name patterns in Claude Code output:
    // "Bash", "Read", "Write", "Edit", "Glob", "Grep", etc.
    let known_tools = [
        "Bash",
        "Read",
        "Write",
        "Edit",
        "Glob",
        "Grep",
        "WebFetch",
        "WebSearch",
        "Agent",
        "NotebookEdit",
    ];
    for tool in &known_tools {
        if text.contains(tool) {
            return tool.to_string();
        }
    }
    "unknown".to_string()
}

/// Strip ANSI escape sequences from text.
#[allow(dead_code)]
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip ESC + '[' + params + final byte
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() || next == '~' {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Extract the meaningful response content from raw CLI terminal output.
///
/// CLI agents (Claude Code, Codex) wrap their output in TUI chrome: prompt
/// boxes (╭─, ╰─), status bars, tool-use headers, spinner lines, etc.
/// This function strips all that, returning just the substantive response text.
///
/// Strategy:
/// 1. Strip ANSI escape sequences
/// 2. Remove TUI chrome lines (box-drawing, status bars, prompts)
/// 3. Remove tool-use header lines ("Read", "Write", "Bash", etc.)
/// 4. Trim leading/trailing whitespace
#[allow(dead_code)]
pub(crate) fn extract_response(output: &str, cli_type: &CliType) -> String {
    let profile = profile_for(cli_type);
    let clean = strip_ansi(output);
    let mut response_lines: Vec<&str> = Vec::new();
    let mut in_response = false;
    let mut found_any_content = false;

    for line in clean.lines() {
        let trimmed = line.trim();

        // Skip empty lines at the start
        if !found_any_content && trimmed.is_empty() {
            continue;
        }

        // Skip TUI box-drawing chrome
        if is_tui_chrome(trimmed) {
            continue;
        }

        // Skip spinner/status lines
        if is_status_line(trimmed) {
            continue;
        }

        // Skip tool-use headers (Claude Code shows "Read file.rs", "Bash ls -la", etc.)
        if is_tool_header(trimmed) {
            // Tool use is part of work, not the response — skip but don't end response
            continue;
        }

        // Skip prompt lines (❯, input boxes)
        if is_prompt_line(trimmed, profile) {
            if in_response {
                // Prompt after response content means response ended
                break;
            }
            continue;
        }

        // Skip processing indicators
        if is_processing_indicator(trimmed, profile) {
            continue;
        }

        // Skip tmux session setup commands (env sanitization, CLI launch)
        if is_setup_command(trimmed) {
            continue;
        }

        // Skip CLI launch commands (codex exec, claude -p, etc.)
        if is_cli_launch_command(trimmed) {
            continue;
        }

        // Skip shell setup lines (export, unset, source)
        if is_shell_setup_line(trimmed) {
            continue;
        }

        // This line is actual content
        found_any_content = true;
        in_response = true;
        response_lines.push(line);
    }

    let result = response_lines.join("\n");
    result.trim().to_string()
}

#[allow(dead_code)]
fn is_tui_chrome(line: &str) -> bool {
    // Box-drawing characters used by CLI TUIs
    let chrome_starts = ["╭", "╰", "├", "└", "┌", "┐", "┘", "┤", "┼", "│"];
    for s in &chrome_starts {
        if line.starts_with(s) {
            return true;
        }
    }
    // Lines that are entirely box-drawing / dashes / whitespace
    if !line.is_empty()
        && line.chars().all(|c| {
            c == '─'
                || c == '━'
                || c == '═'
                || c == '│'
                || c == '┃'
                || c == '╭'
                || c == '╮'
                || c == '╰'
                || c == '╯'
                || c == '├'
                || c == '┤'
                || c == '┬'
                || c == '┴'
                || c == '-'
                || c == '='
                || c == ' '
        })
    {
        return true;
    }
    false
}

#[allow(dead_code)]
fn is_status_line(line: &str) -> bool {
    // Claude Code status bar patterns
    line.contains("tokens") && (line.contains("input") || line.contains("output"))
        || line.contains("Cost:")
        || line.contains("Duration:")
        || (line.contains("Model:") && line.contains("/"))
        || line.starts_with("Session:")
        || line.starts_with("Context:")
}

#[allow(dead_code)]
fn is_tool_header(line: &str) -> bool {
    // Claude Code tool-use indicators shown inline
    let tools = [
        "Read ",
        "Write ",
        "Edit ",
        "Bash ",
        "Glob ",
        "Grep ",
        "WebFetch ",
        "WebSearch ",
        "Agent ",
        "NotebookEdit ",
        "Running ",
        "Reading ",
        "Writing ",
        "Editing ",
        "Searching ",
    ];
    for t in &tools {
        if line.starts_with(t) && line.len() < 200 {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn is_prompt_line(line: &str, profile: &CliProfile) -> bool {
    let trimmed = line.trim();
    for p in profile.input_prompt_patterns {
        if line.contains(p) || trimmed.contains(p) {
            return true;
        }
        // Also match if the trimmed line equals the pattern with trailing space stripped
        let pattern_trimmed = p.trim();
        if !pattern_trimmed.is_empty() && trimmed == pattern_trimmed {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
fn is_processing_indicator(line: &str, profile: &CliProfile) -> bool {
    // Only match lines that are JUST a processing indicator (short lines)
    if line.len() > 80 {
        return false;
    }
    for p in profile.processing_patterns {
        if line.contains(p) && line.len() < 60 {
            return true;
        }
    }
    false
}

/// Detect tmux session setup commands injected during environment sanitization.
#[allow(dead_code)]
fn is_setup_command(line: &str) -> bool {
    // These are injected by session::create_session to sanitize the CLI environment
    line.starts_with("unset CLAUDECODE")
        || line.starts_with("unset CLAUDE_CODE")
        || line.starts_with("unset CODEX")
        || line.starts_with("unset OPENAI_")
        || line.starts_with("unset ANTHROPIC_")
        || (line.starts_with("export ") && line.contains("HARMONIA"))
}

/// Detect CLI tool launch commands (codex exec, claude -p, etc.).
#[allow(dead_code)]
fn is_cli_launch_command(line: &str) -> bool {
    (line.starts_with("codex ") || line.starts_with("codex exec"))
        || (line.starts_with("claude ") && line.contains(" -p "))
        || (line.starts_with("claude ") && line.contains("--print"))
        || line.starts_with("$(cat /tmp/harmonia-prompt-")
}

/// Detect shell setup lines (export, unset, source commands).
#[allow(dead_code)]
fn is_shell_setup_line(line: &str) -> bool {
    (line.starts_with("export ") && !line.contains("=''"))
        || line.starts_with("unset ")
        || line.starts_with("source ")
        || line.starts_with(". ")
        || (line.starts_with("cat ") && line.contains("/tmp/harmonia-prompt-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[32mHello\x1b[0m World";
        assert_eq!(strip_ansi(input), "Hello World");
    }

    #[test]
    fn test_detect_processing() {
        let output = "Some output\nThinking...\n";
        let state = detect_state(output, &CliType::ClaudeCode);
        assert!(matches!(state, CliState::Processing));
    }

    #[test]
    fn test_detect_input_prompt() {
        let output = "Previous output\n\n❯ ";
        let state = detect_state(output, &CliType::ClaudeCode);
        assert!(matches!(state, CliState::WaitingForInput));
    }

    #[test]
    fn test_detect_permission() {
        let output = "The Bash tool wants to run:\nls -la\nAllow this? (y/n)\nAllow Deny";
        let state = detect_state(output, &CliType::ClaudeCode);
        assert!(matches!(state, CliState::WaitingForPermission { .. }));
    }

    #[test]
    fn test_extract_response_strips_chrome() {
        let output = "╭─────────────────────────────────╮\n\
                       │ test prompt                      │\n\
                       ╰─────────────────────────────────╯\n\
                       \n\
                       This is the actual response text.\n\
                       It spans multiple lines.\n\
                       \n\
                       ❯ ";
        let extracted = extract_response(output, &CliType::ClaudeCode);
        assert!(extracted.contains("This is the actual response text."));
        assert!(extracted.contains("It spans multiple lines."));
        assert!(!extracted.contains("╭"));
        assert!(!extracted.contains("╰"));
        assert!(!extracted.contains("❯"));
    }

    #[test]
    fn test_extract_response_skips_tool_headers() {
        let output = "Reading file.rs\n\
                       Bash ls -la\n\
                       \n\
                       Here is the answer to your question.\n\
                       \n\
                       ❯ ";
        let extracted = extract_response(output, &CliType::ClaudeCode);
        assert!(extracted.contains("Here is the answer"));
        assert!(!extracted.contains("Reading file.rs"));
    }
}
