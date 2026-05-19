//! Single declarative slash-command registry.
//!
//! Every command the gateway recognises is declared here once. The
//! [`Handler`] variant decides how the gateway routes it:
//!
//! * [`Handler::Native`] — Rust handles the command in-process and returns a
//!   response string the gateway sends back to the originating frontend.
//! * [`Handler::PassThrough`] — the gateway leaves the envelope alone; the
//!   orchestrator (Lisp) receives the unmodified `/<command>` text and
//!   handles it via `src/core/system-commands.lisp`. The gateway emits no
//!   response of its own.
//! * [`Handler::Exit`] — special: signals graceful shutdown.
//!
//! There is no third "stub-respond" variant. Either the gateway answers, or
//! it stays out of the way. This is the cleanup of the previous
//! Native/Delegated fork where `Delegated` produced a placeholder string the
//! user actually saw instead of the real handler's output.

use crate::model::SecurityLabel;

/// Descriptor for a single gateway command.
pub(crate) struct CommandMeta {
    /// The slash-command string, e.g. `"/wallet"`.
    pub name: &'static str,
    /// How this command is executed.
    pub handler: Handler,
    /// Minimum security label required (`None` = unrestricted).
    pub min_security: Option<fn(SecurityLabel) -> bool>,
    /// If `true`, only the TUI frontend may invoke this command.
    pub tui_only: bool,
}

/// Routing decision for a command.
pub(crate) enum Handler {
    /// Gateway handles in-process. Args are the trimmed tail of the command.
    Native(fn(&str) -> String),
    /// Gateway leaves the envelope alone — Lisp orchestrator handles it.
    /// Use this for commands implemented in `src/core/system-commands.lisp`.
    PassThrough,
    /// Gateway signals graceful shutdown.
    Exit,
}

fn is_read_allowed(label: SecurityLabel) -> bool {
    matches!(label, SecurityLabel::Owner | SecurityLabel::Authenticated)
}

pub(crate) static ALL_COMMANDS: &[CommandMeta] = &[
    // ── Native (gateway-handled in Rust) ─────────────────────────────────
    CommandMeta {
        name: "/help",
        handler: Handler::Native(super::native::execute_help),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/wallet",
        handler: Handler::Native(super::native::execute_wallet),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/identity",
        handler: Handler::Native(super::native::execute_identity),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/auto",
        handler: Handler::Native(super::native::execute_tier_auto),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/eco",
        handler: Handler::Native(super::native::execute_tier_eco),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/premium",
        handler: Handler::Native(super::native::execute_tier_premium),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/free",
        handler: Handler::Native(super::native::execute_tier_free),
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    // ── Pass-through to the Lisp orchestrator ────────────────────────────
    // These are handled by `src/core/system-commands.lisp`. The gateway must
    // not generate a response — it would race the Lisp output and confuse
    // the frontend.
    CommandMeta {
        name: "/session-create",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-list",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-current",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-events",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/session-append",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/status",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/backends",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/frontends",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/tools",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/chronicle",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/metrics",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/security",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/feedback",
        handler: Handler::PassThrough,
        min_security: None,
        tui_only: false,
    },
    CommandMeta {
        name: "/route",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    CommandMeta {
        name: "/policies",
        handler: Handler::PassThrough,
        min_security: Some(is_read_allowed),
        tui_only: false,
    },
    // ── Special ──────────────────────────────────────────────────────────
    CommandMeta {
        name: "/exit",
        handler: Handler::Exit,
        min_security: None,
        tui_only: true,
    },
];

/// Look up a command by its slash-name (case-insensitive).
/// Returns the [`CommandMeta`] and the trimmed argument tail.
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
