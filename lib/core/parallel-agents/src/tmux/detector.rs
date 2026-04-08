//! Terminal output state detection.
//!
//! Examines captured tmux pane output and determines what state the CLI
//! agent is in. This is the "ears" of the swarm -- it listens to each
//! agent's terminal and reports its harmonic state.
//!
//! Detection is priority-ordered: error > completion > permission >
//! confirmation > selection > processing > input. Higher-priority states
//! override lower ones when multiple patterns match.

use super::cli_profiles::{profile_for, CliProfile};
use super::detector_impl::ansi::strip_ansi;
use super::detector_impl::filters::{
    is_cli_launch_command, is_processing_indicator, is_prompt_line, is_setup_command,
    is_shell_setup_line, is_status_line, is_tool_header, is_tui_chrome,
};
use super::detector_impl::state_checks::{
    detect_completion, detect_confirmation, detect_error, detect_input_prompt, detect_onboarding,
    detect_permission, detect_plan_mode, detect_processing, detect_selection,
};
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
        return CliState::Error(format!("[{}] {}", profile.name, error));
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

/// Extract the meaningful response content from raw CLI terminal output.
///
/// CLI agents (Claude Code, Codex) wrap their output in TUI chrome: prompt
/// boxes, status bars, tool-use headers, spinner lines, etc.
/// This function strips all that, returning just the substantive response text.
///
/// Strategy:
/// 1. Strip ANSI escape sequences
/// 2. Remove TUI chrome lines (box-drawing, status bars, prompts)
/// 3. Remove tool-use header lines ("Read", "Write", "Bash", etc.)
/// 4. Trim leading/trailing whitespace
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
            // Tool use is part of work, not the response -- skip but don't end response
            continue;
        }

        // Skip prompt lines
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let output = "\u{256d}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}\n\
                       \u{2502} test prompt                      \u{2502}\n\
                       \u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}\n\
                       \n\
                       This is the actual response text.\n\
                       It spans multiple lines.\n\
                       \n\
                       \u{276f} ";
        let extracted = extract_response(output, &CliType::ClaudeCode);
        assert!(extracted.contains("This is the actual response text."));
        assert!(extracted.contains("It spans multiple lines."));
        assert!(!extracted.contains("\u{256d}"));
        assert!(!extracted.contains("\u{2570}"));
        assert!(!extracted.contains("\u{276f}"));
    }

    #[test]
    fn test_extract_response_skips_tool_headers() {
        let output = "Reading file.rs\n\
                       Bash ls -la\n\
                       \n\
                       Here is the answer to your question.\n\
                       \n\
                       \u{276f} ";
        let extracted = extract_response(output, &CliType::ClaudeCode);
        assert!(extracted.contains("Here is the answer"));
        assert!(!extracted.contains("Reading file.rs"));
    }
}
