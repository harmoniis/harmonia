/// Unified command dispatch — the gateway is the single interception point for
/// ALL /commands from ALL frontends (TUI, MQTT, Tailscale, paired nodes).
///
/// Commands are handled in two tiers:
///   1. **Native** — fully executed in Rust (wallet, identity, help).
///   2. **Delegated** — routed via IPC dispatch in the runtime actor system.
///
/// Agent-level prompts pass through to the orchestrator.
use crate::commands::reference::expand_at_references;
use crate::commands::registry::{lookup, CommandKind};
#[cfg(test)]
use crate::commands::registry::ALL_COMMANDS;
use crate::model::{ChannelEnvelope, SecurityLabel};
use crate::registry::Registry;

/// Intercept ALL system commands from the envelope batch.
///
/// For each envelope whose body text matches a known command:
///   1. Enforce security policy (Owner/Authenticated/TUI-only).
///   2. Execute the handler (native Rust or delegated Lisp callback).
///   3. Send the response back to the originating frontend.
///   4. Filter the envelope out so Lisp only receives agent prompts.
///
/// Returns envelopes that were NOT intercepted (pass-through to Lisp).
pub fn intercept_commands(
    registry: &Registry,
    envelopes: Vec<ChannelEnvelope>,
) -> Vec<ChannelEnvelope> {
    let mut pass_through = Vec::with_capacity(envelopes.len());

    for envelope in envelopes {
        match lookup(&envelope.body.text) {
            Some((meta, ref args)) => {
                let response = execute_command(
                    meta,
                    args,
                    envelope.security.label,
                    &envelope.channel.kind,
                );
                match response {
                    CommandResult::Response(text) => {
                        if let Err(e) = crate::baseband::send_signal(
                            registry,
                            &envelope.channel.kind,
                            &envelope.channel.address,
                            &text,
                        ) {
                            log::warn!(
                                "gateway: command response send failed for {}: {}",
                                meta.name,
                                e
                            );
                        }
                    }
                    CommandResult::SystemExit => {
                        crate::state::set_pending_exit(true);
                        let _ = crate::baseband::send_signal(
                            registry,
                            &envelope.channel.kind,
                            &envelope.channel.address,
                            "Session ended.",
                        );
                    }
                }
            }
            None => {
                // Expand @path references before passing to Lisp.
                let mut enriched = envelope;
                enriched.body.text = expand_at_references(&enriched.body.text);
                pass_through.push(enriched);
            }
        }
    }

    pass_through
}

// ── Internal dispatch ────────────────────────────────────────────────

enum CommandResult {
    Response(String),
    SystemExit,
}

fn execute_command(
    meta: &crate::commands::registry::CommandMeta,
    args: &str,
    security: SecurityLabel,
    channel_kind: &str,
) -> CommandResult {
    // Security gate
    if let Some(check) = meta.min_security {
        if !check(security) {
            return CommandResult::Response(format!(
                "[system] Permission denied: {} requires elevated access.",
                meta.name
            ));
        }
    }
    if meta.tui_only && channel_kind != "tui" {
        return CommandResult::Response(format!(
            "[system] {} is only available from the TUI.",
            meta.name
        ));
    }

    // Dispatch by kind — no giant match, the registry carries the handler.
    match &meta.kind {
        CommandKind::Native(handler) => CommandResult::Response(handler(args)),
        CommandKind::Delegated => CommandResult::Response(format!(
            "[system] Command {} is handled by the runtime IPC dispatch.",
            meta.name
        )),
        CommandKind::Exit => CommandResult::SystemExit,
    }
}

// ── Tests ────────────────────────────────────────────────────────────

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

    // ── Routing command tests ────────────────────────────────────────

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
    fn routing_commands_are_native() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            let (meta, _) = lookup(cmd).unwrap();
            assert!(
                matches!(meta.kind, CommandKind::Native(_)),
                "{} should be Native",
                cmd
            );
        }
    }

    #[test]
    fn route_is_not_native() {
        let (meta, _) = lookup("/route").unwrap();
        assert!(
            matches!(meta.kind, CommandKind::Delegated),
            "/route should be Delegated, not Native"
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
    fn tier_commands_denied_for_anonymous() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            let (meta, _) = lookup(cmd).unwrap();
            let result = execute_command(meta, "", SecurityLabel::Anonymous, "mqtt");
            match result {
                CommandResult::Response(msg) => {
                    assert!(msg.contains("Permission denied"), "{}: {}", cmd, msg);
                }
                _ => panic!("{} should deny anonymous", cmd),
            }
        }
    }

    #[test]
    fn tier_commands_allowed_for_owner_on_mqtt() {
        let (meta, _) = lookup("/eco").unwrap();
        let result = execute_command(meta, "", SecurityLabel::Owner, "mqtt");
        match result {
            CommandResult::Response(msg) => {
                assert!(
                    msg.contains("Routing tier: eco"),
                    "expected tier confirmation: {}",
                    msg
                );
            }
            _ => panic!("/eco should work for Owner on mqtt"),
        }
    }

    // ── @reference expansion tests ──────────────────────────────────

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
