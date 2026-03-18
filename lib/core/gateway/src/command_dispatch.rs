/// Unified command dispatch — the gateway is the single interception point for
/// ALL /commands from ALL frontends (TUI, MQTT, Tailscale, paired nodes).
///
/// Commands are handled in two tiers:
///   1. **Native** — fully executed in Rust (wallet, identity, help).
///   2. **Delegated** — routed to a Lisp-registered callback that has access
///      to runtime state (status, backends, chronicle, etc.).
///
/// Lisp never sees command envelopes; only agent-level prompts pass through.
use crate::model::{ChannelEnvelope, SecurityLabel};
use crate::registry::Registry;
use std::ffi::{CStr, CString};

extern "C" {
    fn free(ptr: *mut std::ffi::c_void);
}

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
];

/// Commands handled entirely in Rust.
const NATIVE_COMMANDS: &[&str] = &["/wallet", "/identity", "/help"];

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
    args: &str,
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
            _ => unreachable!(),
        });
    }

    // Delegated to Lisp callback
    match query_delegated(command, args) {
        Some(response) if response == ":system-exit" => CommandResult::SystemExit,
        Some(response) => CommandResult::Response(response),
        None => CommandResult::Response(format!(
            "[system] Command handler not available for {command}. \
             Lisp command query callback may not be registered yet."
        )),
    }
}

// ── Lisp callback delegation ──────────────────────────────────────────────

fn query_delegated(command: &str, args: &str) -> Option<String> {
    let handler = crate::state::command_query()?;
    let c_command = CString::new(command).ok()?;
    let c_args = CString::new(args).ok()?;
    let result_ptr = unsafe { handler(c_command.as_ptr(), c_args.as_ptr()) };
    if result_ptr.is_null() {
        return None;
    }
    let result = unsafe { CStr::from_ptr(result_ptr) }
        .to_string_lossy()
        .into_owned();
    unsafe { free(result_ptr as *mut std::ffi::c_void) };
    Some(result)
}

// ── Native command handlers ───────────────────────────────────────────────

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
                pass_through.push(envelope);
            }
        }
    }

    pass_through
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
}
