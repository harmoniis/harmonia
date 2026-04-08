/// Command metadata and the static registry of all known commands.
use crate::model::SecurityLabel;

/// Descriptor for a single gateway command.
pub(crate) struct CommandMeta {
    /// The slash-command string, e.g. "/wallet".
    pub name: &'static str,
    /// How this command is executed.
    pub kind: CommandKind,
    /// Minimum security label required (`None` = unrestricted).
    pub min_security: Option<fn(SecurityLabel) -> bool>,
    /// If `true`, only the TUI frontend may invoke this command.
    pub tui_only: bool,
}

/// Execution strategy for a command.
pub(crate) enum CommandKind {
    /// Handled entirely in Rust — the function returns a response string.
    Native(fn(&str) -> String),
    /// Routed to the runtime IPC actor system.
    Delegated,
    /// Special: triggers a graceful shutdown.
    Exit,
}

fn is_read_allowed(label: SecurityLabel) -> bool {
    matches!(label, SecurityLabel::Owner | SecurityLabel::Authenticated)
}

/// Every command the gateway recognises, with metadata.
pub(crate) static ALL_COMMANDS: &[CommandMeta] = &[
    // ── Native commands ──────────────────────────────────────────────
    CommandMeta {
        name: "/help",
        kind: CommandKind::Native(super::native::execute_help),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/wallet",
        kind: CommandKind::Native(super::native::execute_wallet),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/identity",
        kind: CommandKind::Native(super::native::execute_identity),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/auto",
        kind: CommandKind::Native(super::native::execute_tier_auto),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/eco",
        kind: CommandKind::Native(super::native::execute_tier_eco),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/premium",
        kind: CommandKind::Native(super::native::execute_tier_premium),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/free",
        kind: CommandKind::Native(super::native::execute_tier_free),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    // ── Session commands (routed to session actor via IPC) ─────────────
    CommandMeta {
        name: "/session-create",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-list",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-current",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-events",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-append",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    // ── Special commands ─────────────────────────────────────────────
    CommandMeta {
        name: "/exit",
        kind: CommandKind::Exit,
        min_security: None,
        tui_only: true,
    },
    // ── Delegated commands (runtime IPC) ─────────────────────────────
    CommandMeta {
        name: "/status",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/backends",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/frontends",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/tools",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/chronicle",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/metrics",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/security",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/feedback",
        kind: CommandKind::Delegated,
        min_security: None,
        tui_only: false,
    },
    CommandMeta {
        name: "/route",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/policies",
        kind: CommandKind::Delegated,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
];

/// Look up a command by its slash-name (case-insensitive).
/// Returns the `CommandMeta` and the argument tail.
pub(crate) fn lookup(text: &str) -> Option<(&'static CommandMeta, String)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for meta in ALL_COMMANDS {
        if lower == meta.name || lower.starts_with(&format!("{} ", meta.name)) {
            let args = if trimmed.len() > meta.name.len() {
                trimmed[meta.name.len()..].trim().to_string()
            } else {
                String::new()
            };
            return Some((meta, args));
        }
    }
    None
}
