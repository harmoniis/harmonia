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
pub(crate) fn detect_state(output: &str, cli_type: &CliType) -> CliState {
    let profile = profile_for(cli_type);
    detect_with_profile(output, profile)
}

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

    // Priority 3: Permission prompt
    if let Some((tool, desc)) = detect_permission(&clean, profile) {
        return CliState::WaitingForPermission {
            tool_name: tool,
            description: desc,
        };
    }

    // Priority 4: Yes/No confirmation
    if let Some(question) = detect_confirmation(&clean, profile) {
        return CliState::WaitingForConfirmation { question };
    }

    // Priority 5: Selection menu
    if let Some(options) = detect_selection(&clean, profile) {
        return CliState::WaitingForSelection { options };
    }

    // Priority 6: Processing (thinking/working)
    if detect_processing(&clean, profile) {
        return CliState::Processing;
    }

    // Priority 7: Waiting for input
    if detect_input_prompt(&clean, profile) {
        return CliState::WaitingForInput;
    }

    // Default: still processing (no recognizable pattern)
    CliState::Processing
}

fn detect_error(text: &str, profile: &CliProfile) -> Option<String> {
    for pattern in profile.error_patterns {
        if let Some(pos) = text.find(pattern) {
            // Extract the error line
            let rest = &text[pos..];
            let line_end = rest.find('\n').unwrap_or(rest.len());
            return Some(rest[..line_end].to_string());
        }
    }
    None
}

fn detect_completion(text: &str, all_lines: &[&str], profile: &CliProfile) -> bool {
    // Check if the very last non-empty line matches a shell prompt
    let last_non_empty = all_lines.iter().rev().find(|l| !l.trim().is_empty());
    if let Some(last) = last_non_empty {
        let clean_last = strip_ansi(last);
        for pattern in profile.completion_patterns {
            if clean_last.contains(pattern) {
                return true;
            }
        }
    }
    // Also check the tail window
    for pattern in profile.completion_patterns {
        if text.contains(pattern) && text.contains("exited") {
            return true;
        }
    }
    false
}

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

fn detect_processing(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.processing_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

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

/// Try to extract a tool name from permission prompt text.
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
}
