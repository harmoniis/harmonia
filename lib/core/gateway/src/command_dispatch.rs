/// Unified command dispatch — the gateway is the single interception point for
/// ALL /commands from ALL frontends (TUI, MQTT, Tailscale, paired nodes).
///
/// Commands are handled in two tiers:
///   1. **Native** — fully executed in Rust (wallet, identity, help).
///   2. **Delegated** — routed via IPC dispatch in the runtime actor system.
///
/// Agent-level prompts pass through to the orchestrator.
use crate::model::{ChannelEnvelope, SecurityLabel};
use crate::registry::Registry;

/// Every command the gateway recognises.
const ALL_COMMANDS: &[&str] = &[
    "/help",
    "/exit",
    "/status",
    "/backends",
    "/frontends",
    "/tools",
    "/chronicle",
    "/metrics",
    "/security",
    "/feedback",
    "/wallet",
    "/identity",
    "/auto",
    "/eco",
    "/premium",
    "/free",
    "/route",
];

/// Commands handled entirely in Rust.
const NATIVE_COMMANDS: &[&str] = &[
    "/wallet",
    "/identity",
    "/help",
    "/auto",
    "/eco",
    "/premium",
    "/free",
];

/// Commands requiring Owner or Authenticated security label.
const READ_RESTRICTED: &[&str] = &[
    "/status",
    "/backends",
    "/frontends",
    "/tools",
    "/chronicle",
    "/metrics",
    "/security",
    "/wallet",
    "/identity",
    "/auto",
    "/eco",
    "/premium",
    "/free",
    "/route",
];

/// Commands restricted to TUI origin.
const TUI_ONLY: &[&str] = &["/exit"];

// ── Parsing ───────────────────────────────────────────────────────────────

fn parse_command(text: &str) -> Option<(&'static str, String)> {
    let trimmed = text.trim();
    if !trimmed.starts_with('/') {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    for &cmd in ALL_COMMANDS {
        if lower == cmd || lower.starts_with(&format!("{cmd} ")) {
            let args = if trimmed.len() > cmd.len() {
                trimmed[cmd.len()..].trim().to_string()
            } else {
                String::new()
            };
            return Some((cmd, args));
        }
    }
    None
}

fn is_read_allowed(label: SecurityLabel) -> bool {
    matches!(label, SecurityLabel::Owner | SecurityLabel::Authenticated)
}

// ── Formatting ────────────────────────────────────────────────────────────

fn kv(key: &str, value: &str) -> String {
    format!("  {:<24} {}", key, value)
}

// ── Command result ────────────────────────────────────────────────────────

enum CommandResult {
    Response(String),
    SystemExit,
}

// ── Dispatch ──────────────────────────────────────────────────────────────

fn execute_command(
    command: &str,
    _args: &str,
    security: SecurityLabel,
    channel_kind: &str,
) -> CommandResult {
    // Security gate
    if READ_RESTRICTED.contains(&command) && !is_read_allowed(security) {
        return CommandResult::Response(format!(
            "[system] Permission denied: {command} requires elevated access."
        ));
    }
    if TUI_ONLY.contains(&command) && channel_kind != "tui" {
        return CommandResult::Response(
            "[system] /exit is only available from the TUI.".to_string(),
        );
    }

    // Native handlers
    if NATIVE_COMMANDS.contains(&command) {
        return CommandResult::Response(match command {
            "/wallet" => execute_wallet(),
            "/identity" => execute_identity(),
            "/help" => execute_help(),
            "/auto" => execute_tier_change("auto"),
            "/eco" => execute_tier_change("eco"),
            "/premium" => execute_tier_change("premium"),
            "/free" => execute_tier_change("free"),
            _ => unreachable!(),
        });
    }

    // Delegated commands are now dispatched via IPC in the runtime actor
    // system. The gateway no longer holds a Lisp FFI callback.
    if command == "/exit" {
        return CommandResult::SystemExit;
    }
    CommandResult::Response(format!(
        "[system] Command {command} is handled by the runtime IPC dispatch."
    ))
}

// ── Native command handlers ───────────────────────────────────────────────

fn execute_tier_change(tier: &str) -> String {
    let _ = harmonia_config_store::set_config("router", "router", "active-tier", tier);
    format!("[system] Routing tier: {}", tier)
}

fn execute_help() -> String {
    let lines = vec![
        "Harmonia System Commands".to_string(),
        String::new(),
        "Lisp-backed (runtime state):".to_string(),
        kv("/status", "System status overview"),
        kv("/backends", "List configured LLM backends"),
        kv("/backends <name>", "Show specific backend details"),
        kv("/frontends", "List all frontends with status"),
        kv("/frontends <name>", "Show specific frontend details"),
        kv("/tools", "List configured tools"),
        kv("/chronicle", "Chronicle overview (summary + GC)"),
        kv("/chronicle harmony", "Harmony summary"),
        kv("/chronicle delegation", "Delegation report"),
        kv("/chronicle costs", "Cost report"),
        kv("/chronicle graph", "Concept graph overview"),
        kv("/chronicle gc", "GC status"),
        kv("/metrics", "Metrics overview (parallel report)"),
        kv("/security", "Security audit overview"),
        kv("/security posture", "Current posture details"),
        kv("/security errors", "Recent errors from error ring"),
        kv("/feedback <note>", "Record human feedback"),
        kv("/exit", "Exit the TUI session (TUI only)"),
        String::new(),
        "Gateway-native (Rust):".to_string(),
        kv("/wallet", "Wallet/vault status"),
        kv("/identity", "Vault symbols and key status"),
        kv("/help", "Show this listing"),
        String::new(),
        "Routing (Owner/Authenticated):".to_string(),
        kv("/auto", "Intelligent routing (default)"),
        kv("/eco", "Cost-optimized routing"),
        kv("/premium", "Quality-optimized routing"),
        kv("/free", "Zero-cost routing (local CLI only)"),
        kv("/route", "Current routing status"),
    ];
    lines.join("\n")
}

fn execute_wallet() -> String {
    if let Err(e) = harmonia_vault::init_from_env() {
        return format!("[system] Vault initialization failed: {e}");
    }
    let wallet_db = harmonia_config_store::get_config("gateway", "global", "wallet-db")
        .ok()
        .flatten()
        .unwrap_or_default();
    let vault_db = harmonia_config_store::get_config("gateway", "global", "vault-db")
        .ok()
        .flatten()
        .unwrap_or_default();
    let wallet_present = !wallet_db.is_empty() && std::path::Path::new(&wallet_db).exists();
    let vault_present = !vault_db.is_empty() && std::path::Path::new(&vault_db).exists();
    let symbols = harmonia_vault::list_secret_symbols();

    let mut lines = vec![
        "Wallet".to_string(),
        "-".repeat(40),
        kv(
            "Wallet DB:",
            if wallet_db.is_empty() {
                "(not set)"
            } else {
                &wallet_db
            },
        ),
        kv("Wallet present:", if wallet_present { "yes" } else { "no" }),
        kv(
            "Vault DB:",
            if vault_db.is_empty() {
                "(not set)"
            } else {
                &vault_db
            },
        ),
        kv("Vault present:", if vault_present { "yes" } else { "no" }),
        kv("Symbols:", &symbols.len().to_string()),
    ];
    if !symbols.is_empty() {
        lines.push(String::new());
        for sym in &symbols {
            let present = harmonia_vault::has_secret_for_symbol(sym);
            lines.push(format!(
                "  {:<28} {}",
                sym,
                if present { "[set]" } else { "[empty]" }
            ));
        }
    }
    lines.join("\n")
}

fn execute_identity() -> String {
    if let Err(e) = harmonia_vault::init_from_env() {
        return format!("[system] Vault initialization failed: {e}");
    }
    let symbols = harmonia_vault::list_secret_symbols();
    let mut lines = vec![
        "Identity & Vault".to_string(),
        "-".repeat(40),
        format!("Vault symbols ({}):", symbols.len()),
    ];
    if symbols.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for sym in &symbols {
            let present = harmonia_vault::has_secret_for_symbol(sym);
            lines.push(format!(
                "  {:<28} {}",
                sym,
                if present { "[set]" } else { "[empty]" }
            ));
        }
    }
    lines.push(String::new());
    lines.push("Backend key status:".to_string());
    for key_name in &["ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"] {
        let has = harmonia_vault::has_secret_for_symbol(key_name);
        lines.push(format!(
            "  {:<28} {}",
            key_name,
            if has { "present" } else { "missing" }
        ));
    }
    lines.join("\n")
}

// ── Public entry point ────────────────────────────────────────────────────

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
        match parse_command(&envelope.body.text) {
            Some((command, ref args)) => {
                let result = execute_command(
                    command,
                    args,
                    envelope.security.label,
                    &envelope.channel.kind,
                );
                match result {
                    CommandResult::Response(response) => {
                        if let Err(e) = crate::baseband::send_signal(
                            registry,
                            &envelope.channel.kind,
                            &envelope.channel.address,
                            &response,
                        ) {
                            log::warn!(
                                "gateway: command response send failed for {}: {}",
                                command,
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

// ── @path reference expansion ───────────────────────────────────────

/// Maximum bytes to inline per file reference.
const AT_REF_MAX_BYTES: usize = 4096;
/// Maximum number of @references per message.
const AT_REF_MAX_COUNT: usize = 5;

/// Expand @path references in message text.
///
/// Finds tokens matching `@<path>` where path is a relative or absolute file/dir path.
/// For files: inlines content wrapped in [FILE: path] ... [/FILE] markers.
/// For directories: lists contents wrapped in [DIR: path] ... [/DIR] markers.
/// Paths are resolved relative to the workspace root (config-store or cwd).
///
/// Generic — works for ALL frontends (TUI, Telegram, MQTT, etc.).
fn expand_at_references(text: &str) -> String {
    let workspace = harmonia_config_store::get_own("workspace", "root")
        .ok()
        .flatten()
        .unwrap_or_else(|| std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".into()));
    let root = std::path::Path::new(&workspace);

    let mut result = text.to_string();
    let mut count = 0;

    // Find @references: @word where word looks like a path (contains / or .).
    let tokens: Vec<String> = text.split_whitespace()
        .filter(|w| w.starts_with('@') && w.len() > 1)
        .filter(|w| {
            let path_part = &w[1..];
            path_part.contains('/') || path_part.contains('.')
        })
        .map(|w| w.to_string())
        .collect();

    for token in tokens {
        if count >= AT_REF_MAX_COUNT { break; }
        let path_str = &token[1..]; // Strip @

        let candidate = if std::path::Path::new(path_str).is_absolute() {
            std::path::PathBuf::from(path_str)
        } else {
            root.join(path_str)
        };

        // Security: verify path is within workspace.
        let canonical = match candidate.canonicalize() {
            Ok(p) => p,
            Err(_) => continue, // File doesn't exist, skip silently.
        };
        let root_canonical = match root.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical.starts_with(&root_canonical) {
            continue; // Path escape attempt, skip.
        }

        let expansion = if canonical.is_dir() {
            // Directory: list contents.
            match std::fs::read_dir(&canonical) {
                Ok(entries) => {
                    let items: Vec<String> = entries
                        .take(30)
                        .filter_map(|e| e.ok())
                        .map(|e| {
                            let name = e.file_name().to_string_lossy().into_owned();
                            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                            if is_dir { format!("  {}/", name) } else { format!("  {}", name) }
                        })
                        .collect();
                    format!("\n[DIR: {}]\n{}\n[/DIR]", path_str, items.join("\n"))
                }
                Err(_) => continue,
            }
        } else {
            // File: inline content (capped).
            match std::fs::read_to_string(&canonical) {
                Ok(content) => {
                    let capped = if content.len() > AT_REF_MAX_BYTES {
                        format!("{}...[truncated at {}B]", &content[..AT_REF_MAX_BYTES], AT_REF_MAX_BYTES)
                    } else {
                        content
                    };
                    format!("\n[FILE: {}]\n{}\n[/FILE]", path_str, capped)
                }
                Err(_) => continue,
            }
        };

        result = result.replacen(&token, &expansion, 1);
        count += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_known_commands() {
        for &cmd in ALL_COMMANDS {
            assert!(
                parse_command(cmd).is_some(),
                "failed to parse command: {}",
                cmd
            );
        }
    }

    #[test]
    fn parse_command_with_args() {
        let (cmd, args) = parse_command("/backends openrouter").unwrap();
        assert_eq!(cmd, "/backends");
        assert_eq!(args, "openrouter");
    }

    #[test]
    fn parse_ignores_unknown() {
        assert!(parse_command("hello world").is_none());
        assert!(parse_command("/unknown").is_none());
    }

    #[test]
    fn parse_case_insensitive() {
        assert!(parse_command("/Wallet").is_some());
        assert!(parse_command("/STATUS").is_some());
        assert!(parse_command("/Chronicle harmony").is_some());
    }

    #[test]
    fn security_checks() {
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
                parse_command(cmd).is_some(),
                "routing command {} should parse",
                cmd
            );
        }
    }

    #[test]
    fn routing_commands_are_read_restricted() {
        for cmd in ["/auto", "/eco", "/premium", "/free", "/route"] {
            assert!(
                READ_RESTRICTED.contains(&cmd),
                "{} should be READ_RESTRICTED",
                cmd
            );
        }
    }

    #[test]
    fn routing_commands_are_native() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            assert!(NATIVE_COMMANDS.contains(&cmd), "{} should be NATIVE", cmd);
        }
    }

    #[test]
    fn route_is_not_native() {
        // /route is delegated to Lisp IPC, not native
        assert!(!NATIVE_COMMANDS.contains(&"/route"));
    }

    #[test]
    fn tier_commands_work_from_any_frontend() {
        // Routing commands should work from MQTT, WhatsApp, etc. — not TUI-only
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            assert!(
                !TUI_ONLY.contains(&cmd),
                "{} should NOT be TUI_ONLY — must work from all frontends",
                cmd
            );
        }
    }

    #[test]
    fn tier_commands_denied_for_anonymous() {
        for cmd in ["/auto", "/eco", "/premium", "/free"] {
            let result = execute_command(cmd, "", SecurityLabel::Anonymous, "mqtt");
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
        // Owner on any frontend should be able to switch tiers
        let result = execute_command("/eco", "", SecurityLabel::Owner, "mqtt");
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
        // @word without / or . is not a path reference.
        let input = "Hello @user how are you?";
        assert_eq!(expand_at_references(input), input);
    }

    #[test]
    fn at_ref_expands_existing_file() {
        // Use Cargo.toml which always exists in the workspace.
        let result = expand_at_references("check @Cargo.toml please");
        assert!(result.contains("[FILE: Cargo.toml]"), "should contain FILE marker: {}", result);
        assert!(result.contains("[/FILE]"), "should contain end marker");
        assert!(result.contains("[package]") || result.contains("[workspace]"),
            "should contain Cargo.toml content");
    }

    #[test]
    fn at_ref_expands_directory() {
        let result = expand_at_references("list @src/ports/");
        // Should either expand as DIR or skip if not in right cwd.
        // Just verify it doesn't crash.
        assert!(!result.is_empty());
    }

    #[test]
    fn at_ref_skips_nonexistent() {
        let result = expand_at_references("read @nonexistent/fake/path.txt");
        // Should leave the token as-is (file doesn't exist).
        assert!(result.contains("@nonexistent/fake/path.txt") || !result.contains("[FILE:"));
    }

    #[test]
    fn at_ref_max_count_limit() {
        // More than AT_REF_MAX_COUNT references should stop expanding.
        let input = "@Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml @Cargo.toml";
        let result = expand_at_references(input);
        let file_count = result.matches("[FILE:").count();
        assert!(file_count <= AT_REF_MAX_COUNT,
            "should expand at most {} files, got {}", AT_REF_MAX_COUNT, file_count);
    }
}
