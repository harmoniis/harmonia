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

#[allow(dead_code)]
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

    /// Patterns indicating an onboarding/survey/first-run prompt to auto-dismiss.
    pub(crate) onboarding_patterns: &'static [&'static str],

    /// Patterns indicating plan mode (accept/reject plan).
    pub(crate) plan_mode_patterns: &'static [&'static str],

    /// How many trailing lines to examine for state detection.
    pub(crate) detection_window: usize,

    /// Whether the permission prompt is a TUI selector (arrow keys + Enter)
    /// rather than a y/n text prompt. If true, approve = Enter (default selection),
    /// deny = navigate to deny + Enter.
    pub(crate) permission_is_selector: bool,

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
#[allow(dead_code)]
pub(crate) static CLAUDE_CODE_PROFILE: CliProfile = CliProfile {
    name: "claude-code",
    input_prompt_patterns: &[
        "❯ ", // Claude Code's interactive prompt (with trailing space)
        "What would you like to do?",
        "How can I help",
        "Enter your prompt",
        "Type a message",
        "╰─", // prompt box bottom border
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
        "⠏", // spinner frames
        "Working",
        "Reading",
        "Searching",
        "Writing",
        "Editing",
        "Running",
        "Compiling",
        "Analyzing",
        "Generating",
        "Fetching",
        "Installing",
        "Building",
        "Downloading",
    ],
    permission_patterns: &[
        "Allow once",
        "Allow always",
        "Allow",
        "Deny",
        "Do you want to proceed",
        "want to allow",
        "wants to run", // "The Bash tool wants to run:"
        "wants to execute",
        "wants to write",
        "wants to edit",
        "wants to read",
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
        "Select",
        "Choose",
        "Pick",
        "[ ] ",           // checkbox unchecked
        "[x] ",           // checkbox checked
        "(●) ",           // radio selected
        "(○) ",           // radio unselected
        "Use arrow keys", // selection instructions
    ],
    completion_patterns: &[
        "$ ",     // back to shell prompt (bash/linux)
        "% ",     // back to shell prompt (zsh/macOS)
        "❯ exit", // explicit exit in interactive mode
        "Session ended",
        "Goodbye",
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
        "ECONNREFUSED",
        "ETIMEDOUT",
        "spawn ENOENT", // command not found
        "command not found",
    ],
    onboarding_patterns: &[
        "Welcome to Claude",
        "Terms of Service",
        "telemetry",
        "Telemetry",
        "opt in",
        "opt out",
        "first time",
        "Get started",
        "getting started",
        "setup wizard",
        "onboarding",
        "Would you like to enable",
    ],
    plan_mode_patterns: &[
        "Plan:",
        "plan mode",
        "Plan mode",
        "Accept plan",
        "Reject plan",
        "accept this plan",
        "reject this plan",
        "Execute plan",
    ],
    detection_window: 25, // larger window — Claude Code output can be verbose
    // Claude Code permission prompts are TUI selector widgets, not y/n text prompts.
    // "Allow once" is the default (first) option — just press Enter to accept.
    // To deny: arrow Down twice to "Deny", then Enter.
    permission_is_selector: true,
    approve_key: "", // just Enter (default selection = "Allow once")
    deny_key: "",    // handled by select_option(2) for "Deny"
    yes_key: "y",
    no_key: "n",
};

/// OpenAI Codex CLI profile.
#[allow(dead_code)]
pub(crate) static CODEX_PROFILE: CliProfile = CliProfile {
    name: "codex",
    input_prompt_patterns: &["❯", "> ", "Enter a prompt", "What would you like"],
    processing_patterns: &[
        "Thinking", "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", "Working", "Running",
    ],
    permission_patterns: &["Allow", "Deny", "approve", "Approve", "(y/n)"],
    confirmation_patterns: &["(y/n)", "(Y/n)", "Continue?", "Proceed?"],
    selection_patterns: &["Select", "Choose"],
    completion_patterns: &[
        "exited",
        "$ ",          // bash/linux
        "% ",          // zsh/macOS
        "tokens used", // codex exec completion indicator
    ],
    error_patterns: &["Error:", "error:", "FATAL", "panicked"],
    onboarding_patterns: &[],
    plan_mode_patterns: &[],
    detection_window: 12,
    permission_is_selector: false,
    approve_key: "y",
    deny_key: "n",
    yes_key: "y",
    no_key: "n",
};

/// Get the profile for a known CLI type.
#[allow(dead_code)]
pub(crate) fn profile_for(cli_type: &CliType) -> &'static CliProfile {
    match cli_type {
        CliType::ClaudeCode => &CLAUDE_CODE_PROFILE,
        CliType::Codex => &CODEX_PROFILE,
        // Custom CLIs fall back to Claude Code profile as the most comprehensive default.
        CliType::Custom { .. } => &CLAUDE_CODE_PROFILE,
    }
}
