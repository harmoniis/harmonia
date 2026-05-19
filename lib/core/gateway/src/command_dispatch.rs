//! Slash-command interception path for *all* frontends.
//!
//! The gateway is the single point where slash commands are recognised. No
//! frontend (TUI, MQTT, HTTP/3, Slack, …) parses `/<command>` itself; they
//! all submit envelopes whose `body.text` is the raw user input. This
//! module:
//!
//! 1. Looks each envelope up against the static [`registry`] of known
//!    commands.
//! 2. Enforces per-command security gates (`min_security`, `tui_only`)
//!    against the envelope's [`SecurityLabel`] and originating channel.
//! 3. Routes by [`Handler`]:
//!    * `Native` → Rust generates the response, gateway sends it back, the
//!      envelope is filtered out.
//!    * `PassThrough` → envelope is returned unchanged for the Lisp
//!      orchestrator (`src/core/system-commands.lisp`) to handle. The
//!      gateway never produces a response of its own, so there's no
//!      double-output.
//!    * `Exit` → set the pending-exit flag, send a closing message,
//!      filter out.
//! 4. For any text that *isn't* a slash command, the body is `@`-expanded
//!    and the envelope passes through to Lisp as a regular prompt.

use crate::commands::reference::expand_at_references;
use crate::commands::registry::{lookup, CommandMeta, Handler};
#[cfg(test)]
use crate::commands::registry::ALL_COMMANDS;
use crate::model::{ChannelEnvelope, SecurityLabel};
use crate::registry::Registry;

/// Intercept slash commands from a batch of inbound envelopes.
///
/// Returns the envelopes that should still reach the Lisp orchestrator —
/// non-commands and `Handler::PassThrough` commands. Native and Exit
/// commands respond directly through the registry's send path and are
/// filtered out.
pub fn intercept_commands(
    registry: &Registry,
    envelopes: Vec<ChannelEnvelope>,
) -> Vec<ChannelEnvelope> {
    let mut pass_through = Vec::with_capacity(envelopes.len());

    for envelope in envelopes {
        match lookup(&envelope.body.text) {
            None => {
                let mut enriched = envelope;
                enriched.body.text = expand_at_references(&enriched.body.text);
                pass_through.push(enriched);
            }
            Some((meta, args)) => {
                if let Err(reason) =
                    enforce_security(meta, envelope.security.label, &envelope.channel.kind)
                {
                    send_back(registry, &envelope, &reason, meta.name);
                    continue;
                }
                match meta.handler {
                    Handler::Native(f) => {
                        let response = f(&args);
                        send_back(registry, &envelope, &response, meta.name);
                    }
                    Handler::PassThrough => {
                        // Recognised command, but Lisp owns it. Don't expand
                        // @refs — slash commands have their own grammar.
                        pass_through.push(envelope);
                    }
                    Handler::Exit => {
                        crate::state::set_pending_exit(true);
                        send_back(registry, &envelope, "Session ended.", meta.name);
                    }
                }
            }
        }
    }

    pass_through
}

fn enforce_security(
    meta: &CommandMeta,
    security: SecurityLabel,
    channel_kind: &str,
) -> Result<(), String> {
    if let Some(check) = meta.min_security {
        if !check(security) {
            return Err(format!(
                "[system] Permission denied: {} requires elevated access.",
                meta.name
            ));
        }
    }
    if meta.tui_only && channel_kind != "tui" {
        return Err(format!(
            "[system] {} is only available from the TUI.",
            meta.name
        ));
    }
    Ok(())
}

fn send_back(registry: &Registry, envelope: &ChannelEnvelope, text: &str, cmd_name: &str) {
    if let Err(e) = crate::baseband::send_signal(
        registry,
        &envelope.channel.kind,
        &envelope.channel.address,
        text,
    ) {
        log::warn!(
            "gateway: command response send failed for {}: {}",
            cmd_name,
            e
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::reference::AT_REF_MAX_COUNT;

    #[test]
    fn parse_all_known_commands() {
        for meta in ALL_COMMANDS {
            assert!(
                lookup(meta.name).is_some(),
                "failed to parse command: {}",
                meta.name
            );
        }
    }

    #[test]
    fn parse_command_with_args() {
        let (meta, args) = lookup("/backends openrouter").unwrap();
        assert_eq!(meta.name, "/backends");
        assert_eq!(args, "openrouter");
    }

    #[test]
    fn parse_ignores_unknown() {
        assert!(lookup("hello world").is_none());
        assert!(lookup("/unknown").is_none());
    }

    #[test]
    fn parse_case_insensitive() {
        assert!(lookup("/Wallet").is_some());
        assert!(lookup("/STATUS").is_some());
        assert!(lookup("/Chronicle harmony").is_some());
    }

    #[test]
    fn security_checks() {
        fn is_read_allowed(label: SecurityLabel) -> bool {
            matches!(label, SecurityLabel::Owner | SecurityLabel::Authenticated)
        }
        assert!(is_read_allowed(SecurityLabel::Owner));
        assert!(is_read_allowed(SecurityLabel::Authenticated));
        assert!(!is_read_allowed(SecurityLabel::Anonymous));
        assert!(!is_read_allowed(SecurityLabel::Untrusted));
    }

    #[test]
    fn parse_routing_commands() {
        for cmd in ["/auto", "/eco", "/premium", "/free", "/route"] {
            assert!(
                lookup(cmd).is_some(),
                "routing command {} should parse",
                cmd
            );
        }
    }

    #[test]
    fn routing_commands_are_read_restricted() {
        for cmd in ["/auto", "/eco", "/premium", "/free", "/route"] {
            let (meta, _) = lookup(cmd).unwrap();
            assert!(
                meta.min_security.is_some(),
                "{} should be read-restricted",
                cmd
            );
        }
    }

    #[test]
    fn tier_commands_are_native() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            let (meta, _) = lookup(cmd).unwrap();
            assert!(
                matches!(meta.handler, Handler::Native(_)),
                "{} should be Native",
                cmd
            );
        }
    }

    #[test]
    fn route_passes_through_to_lisp() {
        let (meta, _) = lookup("/route").unwrap();
        assert!(
            matches!(meta.handler, Handler::PassThrough),
            "/route should be PassThrough so the Lisp orchestrator handles it"
        );
    }

    #[test]
    fn tier_commands_work_from_any_frontend() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            let (meta, _) = lookup(cmd).unwrap();
            assert!(
                !meta.tui_only,
                "{} should NOT be TUI-only — must work from all frontends",
                cmd
            );
        }
    }

    #[test]
    fn at_ref_no_references() {
        let input = "Hello world, no references here";
        assert_eq!(expand_at_references(input), input);
    }

    #[test]
    fn at_ref_ignores_bare_at() {
        let input = "Hello @user how are you?";
        assert_eq!(expand_at_references(input), input);
    }

    #[test]
    fn at_ref_expands_existing_file() {
        let result = expand_at_references("check @Cargo.toml please");
        assert!(
            result.contains("[FILE: Cargo.toml]"),
            "should contain FILE marker: {}",
            result
        );
        assert!(result.contains("[/FILE]"), "should contain end marker");
        assert!(
            result.contains("[package]") || result.contains("[workspace]"),
            "should contain Cargo.toml content"
        );
    }

    #[test]
    fn at_ref_expands_directory() {
        let result = expand_at_references("list @src/ports/");
        assert!(!result.is_empty());
    }

    #[test]
    fn at_ref_skips_nonexistent() {
        let result = expand_at_references("read @nonexistent/fake/path.txt");
        assert!(result.contains("@nonexistent/fake/path.txt") || !result.contains("[FILE:"));
    }

    #[test]
    fn at_ref_max_count_limit() {
        let input =
            "@Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml";
        let result = expand_at_references(input);
        let file_count = result.matches("[FILE:").count();
        assert!(
            file_count <= AT_REF_MAX_COUNT,
            "should expand at most {} files, got {}",
            AT_REF_MAX_COUNT,
            file_count
        );
    }
}
