use crate::tmux::cli_profiles::CliProfile;

pub(crate) fn detect_error(text: &str, profile: &CliProfile) -> Option<String> {
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

pub(crate) fn detect_completion(text: &str, all_lines: &[&str], profile: &CliProfile) -> bool {
    // Check if the very last non-empty line looks like a shell prompt.
    // Shell prompts typically end with "$ " or "% " (possibly preceded by
    // username, hostname, path, or other decorations).
    let last_non_empty = all_lines.iter().rev().find(|l| !l.trim().is_empty());
    if let Some(last) = last_non_empty {
        let clean_last = super::ansi::strip_ansi(last).trim().to_string();
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

pub(crate) fn detect_permission(text: &str, profile: &CliProfile) -> Option<(String, String)> {
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

pub(crate) fn detect_confirmation(text: &str, profile: &CliProfile) -> Option<String> {
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

pub(crate) fn detect_selection(text: &str, profile: &CliProfile) -> Option<Vec<String>> {
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

pub(crate) fn detect_processing(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.processing_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

pub(crate) fn detect_input_prompt(text: &str, profile: &CliProfile) -> bool {
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

pub(crate) fn detect_onboarding(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.onboarding_patterns {
        if text.contains(pattern) {
            return true;
        }
    }
    false
}

pub(crate) fn detect_plan_mode(text: &str, profile: &CliProfile) -> bool {
    for pattern in profile.plan_mode_patterns {
        if text.contains(pattern) {
            return true;
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
