//! CLI-specific profiles for tmux agent detection.
//!
//! Each profile defines the patterns that identify what state a CLI is in
//! by examining its terminal output. Profiles are evolvable — the initial
//! set is hardcoded for Claude Code and Codex, but new profiles can be
//! registered at runtime for custom CLIs.
//!
//! The profile architecture follows harmonic self-similarity: every CLI
//! agent is a "voice" with the same interface but unique timbre.

use crate::model::CliType;

#[derive(Clone, Debug)]
pub(crate) struct CliProfile {
    #[allow(dead_code)]
    pub(crate) name: &'static str,

    /// Patterns indicating the CLI is waiting for free-text user input.
    /// Matched against the last N lines of terminal output.
    pub(crate) input_prompt_patterns: &'static [&'static str],

    /// Patterns indicating the CLI is processing (thinking/working).
    pub(crate) processing_patterns: &'static [&'static str],

    /// Patterns indicating a permission/approval prompt.
    pub(crate) permission_patterns: &'static [&'static str],

    /// Patterns indicating a yes/no confirmation prompt.
    pub(crate) confirmation_patterns: &'static [&'static str],

    /// Patterns indicating a selection/choice menu.
    pub(crate) selection_patterns: &'static [&'static str],

    /// Patterns indicating the CLI has finished and exited.
    pub(crate) completion_patterns: &'static [&'static str],

    /// Patterns indicating an error state.
    pub(crate) error_patterns: &'static [&'static str],

    /// How many trailing lines to examine for state detection.
    pub(crate) detection_window: usize,

    /// Key to send for "approve/allow" permission prompts.
    pub(crate) approve_key: &'static str,

    /// Key to send for "deny" permission prompts.
    pub(crate) deny_key: &'static str,

    /// Key to send for "yes" confirmation.
    pub(crate) yes_key: &'static str,

    /// Key to send for "no" confirmation.
    pub(crate) no_key: &'static str,
}

/// Claude Code CLI profile.
pub(crate) static CLAUDE_CODE_PROFILE: CliProfile = CliProfile {
    name: "claude-code",
    input_prompt_patterns: &[
        "❯",  // Claude Code's default prompt
        "> ", // fallback prompt
        "What would you like to do?",
        "How can I help",
        "Enter your prompt",
        "Type a message",
        "╰─", // prompt box bottom
    ],
    processing_patterns: &[
        "Thinking",
        "⠋",
        "⠙",
        "⠹",
        "⠸",
        "⠼",
        "⠴",
        "⠦",
        "⠧",
        "⠇",
        "⠏", // spinner
        "Working",
        "Reading",
        "Searching",
        "Writing",
        "Editing",
        "Running",
    ],
    permission_patterns: &[
        "Allow",
        "Deny",
        "approve",
        "Do you want to proceed",
        "Permission",
        "Allow once",
        "Allow always",
        "want to allow",
        "(y/n)",
        "Yes / No",
    ],
    confirmation_patterns: &[
        "(y/n)",
        "(Y/n)",
        "(yes/no)",
        "Continue?",
        "Proceed?",
        "Are you sure",
        "Confirm",
    ],
    selection_patterns: &[
        "Select", "Choose", "Pick", "❯",   // selection cursor (context-dependent)
        "[ ]", // checkbox
        "[x]", // checked checkbox
        "(●)", // radio selected
        "(○)", // radio unselected
    ],
    completion_patterns: &[
        "exited",
        "Session ended",
        "Goodbye",
        "$ ", // back to shell prompt (bash/linux)
        "% ", // back to shell prompt (zsh/macOS)
    ],
    error_patterns: &[
        "Error:",
        "error:",
        "ERROR",
        "fatal:",
        "panicked",
        "Connection refused",
        "API key",
        "rate limit",
        "timed out",
    ],
    detection_window: 15,
    approve_key: "y",
    deny_key: "n",
    yes_key: "y",
    no_key: "n",
};

/// OpenAI Codex CLI profile.
pub(crate) static CODEX_PROFILE: CliProfile = CliProfile {
    name: "codex",
    input_prompt_patterns: &["❯", "> ", "Enter a prompt", "What would you like"],
    processing_patterns: &[
        "Thinking", "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "Working", "Running",
    ],
    permission_patterns: &["Allow", "Deny", "approve", "Approve", "(y/n)"],
    confirmation_patterns: &["(y/n)", "(Y/n)", "Continue?", "Proceed?"],
    selection_patterns: &["Select", "Choose", "❯"],
    completion_patterns: &[
        "exited",
        "$ ",          // bash/linux
        "% ",          // zsh/macOS
        "tokens used", // codex exec completion indicator
    ],
    error_patterns: &["Error:", "error:", "FATAL", "panicked"],
    detection_window: 12,
    approve_key: "y",
    deny_key: "n",
    yes_key: "y",
    no_key: "n",
};

/// Get the profile for a known CLI type.
pub(crate) fn profile_for(cli_type: &CliType) -> &'static CliProfile {
    match cli_type {
        CliType::ClaudeCode => &CLAUDE_CODE_PROFILE,
        CliType::Codex => &CODEX_PROFILE,
        // Custom CLIs fall back to Claude Code profile as the most comprehensive default.
        CliType::Custom { .. } => &CLAUDE_CODE_PROFILE,
    }
}
