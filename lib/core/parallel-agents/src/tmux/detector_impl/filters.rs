use crate::tmux::cli_profiles::CliProfile;

pub(crate) fn is_tui_chrome(line: &str) -> bool {
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

pub(crate) fn is_status_line(line: &str) -> bool {
    // Claude Code status bar patterns
    line.contains("tokens") && (line.contains("input") || line.contains("output"))
        || line.contains("Cost:")
        || line.contains("Duration:")
        || (line.contains("Model:") && line.contains("/"))
        || line.starts_with("Session:")
        || line.starts_with("Context:")
}

pub(crate) fn is_tool_header(line: &str) -> bool {
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

pub(crate) fn is_prompt_line(line: &str, profile: &CliProfile) -> bool {
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

pub(crate) fn is_processing_indicator(line: &str, profile: &CliProfile) -> bool {
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
pub(crate) fn is_setup_command(line: &str) -> bool {
    // These are injected by session::create_session to sanitize the CLI environment
    line.starts_with("unset CLAUDECODE")
        || line.starts_with("unset CLAUDE_CODE")
        || line.starts_with("unset CODEX")
        || line.starts_with("unset OPENAI_")
        || line.starts_with("unset ANTHROPIC_")
        || (line.starts_with("export ") && line.contains("HARMONIA"))
}

/// Detect CLI tool launch commands (codex exec, claude -p, etc.).
pub(crate) fn is_cli_launch_command(line: &str) -> bool {
    (line.starts_with("codex ") || line.starts_with("codex exec"))
        || (line.starts_with("claude ") && line.contains(" -p "))
        || (line.starts_with("claude ") && line.contains("--print"))
        || line.starts_with("$(cat /tmp/harmonia-prompt-")
}

/// Detect shell setup lines (export, unset, source commands).
pub(crate) fn is_shell_setup_line(line: &str) -> bool {
    (line.starts_with("export ") && !line.contains("=''"))
        || line.starts_with("unset ")
        || line.starts_with("source ")
        || line.starts_with(". ")
        || (line.starts_with("cat ") && line.contains("/tmp/harmonia-prompt-"))
}
