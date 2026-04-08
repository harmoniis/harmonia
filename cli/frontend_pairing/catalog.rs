//! Frontend field definitions and credential prompting catalog.

use dialoguer::{Input, Password};
use harmonia_node_rpc::FrontendConfigEntry;

pub(crate) struct FrontendField {
    pub key: &'static str,
    pub prompt: &'static str,
    pub default: Option<&'static str>,
    pub secret: bool,
    pub optional: bool,
}

pub(crate) struct FrontendCatalogEntry {
    pub name: &'static str,
    pub display: &'static str,
    pub fields: &'static [FrontendField],
}

pub(crate) const TELEGRAM_FIELDS: &[FrontendField] = &[FrontendField {
    key: "bot-token", prompt: "Telegram bot token", default: None, secret: true, optional: false,
}];

pub(crate) const SLACK_FIELDS: &[FrontendField] = &[
    FrontendField { key: "bot-token", prompt: "Slack bot token", default: None, secret: true, optional: false },
    FrontendField { key: "app-token", prompt: "Slack app token", default: None, secret: true, optional: false },
    FrontendField { key: "channels", prompt: "Slack channel IDs (comma-separated)", default: None, secret: false, optional: false },
];

pub(crate) const DISCORD_FIELDS: &[FrontendField] = &[
    FrontendField { key: "bot-token", prompt: "Discord bot token", default: None, secret: true, optional: false },
    FrontendField { key: "channels", prompt: "Discord channel IDs (comma-separated)", default: None, secret: false, optional: false },
];

pub(crate) const MATTERMOST_FIELDS: &[FrontendField] = &[
    FrontendField { key: "api-url", prompt: "Mattermost API URL", default: None, secret: false, optional: false },
    FrontendField { key: "bot-token", prompt: "Mattermost bot token", default: None, secret: true, optional: false },
    FrontendField { key: "channels", prompt: "Mattermost channel IDs (comma-separated)", default: None, secret: false, optional: false },
];

pub(crate) const WHATSAPP_FIELDS: &[FrontendField] = &[
    FrontendField { key: "api-url", prompt: "WhatsApp bridge URL", default: Some("http://127.0.0.1:3000"), secret: false, optional: false },
    FrontendField { key: "api-key", prompt: "WhatsApp bridge API key", default: None, secret: true, optional: true },
];

pub(crate) const SIGNAL_FIELDS: &[FrontendField] = &[
    FrontendField { key: "rpc-url", prompt: "Signal bridge URL", default: Some("http://127.0.0.1:8080"), secret: false, optional: false },
    FrontendField { key: "auth-token", prompt: "Signal bridge auth token", default: None, secret: true, optional: true },
];

#[cfg(target_os = "macos")]
pub(crate) const IMESSAGE_FIELDS: &[FrontendField] = &[
    FrontendField { key: "server-url", prompt: "BlueBubbles server URL", default: None, secret: false, optional: false },
    FrontendField { key: "password", prompt: "BlueBubbles password", default: None, secret: true, optional: true },
];

pub(crate) const EMAIL_FIELDS: &[FrontendField] = &[
    FrontendField { key: "imap-host", prompt: "IMAP host", default: None, secret: false, optional: false },
    FrontendField { key: "imap-port", prompt: "IMAP port", default: Some("993"), secret: false, optional: false },
    FrontendField { key: "imap-user", prompt: "IMAP username", default: None, secret: false, optional: false },
    FrontendField { key: "imap-password", prompt: "IMAP password", default: None, secret: true, optional: false },
    FrontendField { key: "imap-mailbox", prompt: "IMAP mailbox", default: Some("INBOX"), secret: false, optional: false },
    FrontendField { key: "imap-tls", prompt: "IMAP TLS (true/false)", default: Some("true"), secret: false, optional: false },
    FrontendField { key: "smtp-host", prompt: "SMTP host", default: None, secret: false, optional: false },
    FrontendField { key: "smtp-port", prompt: "SMTP port", default: Some("587"), secret: false, optional: false },
    FrontendField { key: "smtp-user", prompt: "SMTP username", default: None, secret: false, optional: false },
    FrontendField { key: "smtp-password", prompt: "SMTP password (Enter to reuse IMAP password)", default: None, secret: true, optional: true },
    FrontendField { key: "smtp-from", prompt: "SMTP from address", default: None, secret: false, optional: false },
    FrontendField { key: "smtp-tls", prompt: "SMTP TLS mode (starttls/tls/none)", default: Some("starttls"), secret: false, optional: false },
];

pub(crate) const NOSTR_FIELDS: &[FrontendField] = &[
    FrontendField { key: "private-key", prompt: "Nostr private key", default: None, secret: true, optional: false },
    FrontendField { key: "relays", prompt: "Nostr relays (comma-separated)", default: Some("wss://relay.damus.io,wss://relay.primal.net,wss://nos.lol"), secret: false, optional: true },
];

pub(crate) const TAILSCALE_FIELDS: &[FrontendField] = &[FrontendField {
    key: "auth-key", prompt: "Tailscale auth key", default: None, secret: true, optional: false,
}];

pub(crate) const HTTP2_FIELDS: &[FrontendField] = &[
    FrontendField { key: "bind", prompt: "HTTP/2 bind address", default: Some("127.0.0.1:9443"), secret: false, optional: false },
    FrontendField { key: "ca-cert", prompt: "Client CA certificate path", default: None, secret: false, optional: false },
    FrontendField { key: "server-cert", prompt: "Server certificate path", default: None, secret: false, optional: false },
    FrontendField { key: "server-key", prompt: "Server private key path", default: None, secret: false, optional: false },
    FrontendField { key: "trusted-client-fingerprints", prompt: "Trusted client identity fingerprints (comma-separated)", default: None, secret: false, optional: false },
    FrontendField { key: "max-concurrent-streams", prompt: "Max concurrent streams", default: Some("64"), secret: false, optional: false },
    FrontendField { key: "session-idle-timeout-ms", prompt: "Session idle timeout (ms)", default: Some("300000"), secret: false, optional: false },
    FrontendField { key: "max-frame-bytes", prompt: "Max frame bytes", default: Some("65536"), secret: false, optional: false },
];

pub(crate) fn frontend_catalog() -> Vec<FrontendCatalogEntry> {
    let mut entries = vec![
        FrontendCatalogEntry { name: "telegram", display: "Telegram", fields: TELEGRAM_FIELDS },
        FrontendCatalogEntry { name: "slack", display: "Slack", fields: SLACK_FIELDS },
        FrontendCatalogEntry { name: "discord", display: "Discord", fields: DISCORD_FIELDS },
        FrontendCatalogEntry { name: "mattermost", display: "Mattermost", fields: MATTERMOST_FIELDS },
        FrontendCatalogEntry { name: "whatsapp", display: "WhatsApp", fields: WHATSAPP_FIELDS },
        FrontendCatalogEntry { name: "signal", display: "Signal", fields: SIGNAL_FIELDS },
        FrontendCatalogEntry { name: "email", display: "Email", fields: EMAIL_FIELDS },
        FrontendCatalogEntry { name: "nostr", display: "Nostr", fields: NOSTR_FIELDS },
        FrontendCatalogEntry { name: "tailscale", display: "Tailscale", fields: TAILSCALE_FIELDS },
        FrontendCatalogEntry { name: "http2", display: "HTTP/2 mTLS", fields: HTTP2_FIELDS },
    ];
    #[cfg(target_os = "macos")]
    entries.push(FrontendCatalogEntry { name: "imessage", display: "iMessage", fields: IMESSAGE_FIELDS });
    entries
}

pub(crate) fn prompt_frontend_values(
    frontend: &FrontendCatalogEntry,
) -> Result<Vec<FrontendConfigEntry>, Box<dyn std::error::Error>> {
    eprintln!(
        "\n  {BOLD_CYAN}{RESET} {BOLD}Configure {}{RESET}\n",
        frontend.display,
        BOLD_CYAN = super::BOLD_CYAN,
        BOLD = super::BOLD,
        RESET = super::RESET,
    );
    let mut values = Vec::new();
    for field in frontend.fields {
        let value = if field.secret {
            Password::new()
                .with_prompt(field.prompt)
                .allow_empty_password(field.optional || field.default.is_some())
                .interact()?
        } else {
            let mut input = Input::<String>::new().with_prompt(field.prompt);
            if let Some(default) = field.default {
                input = input.default(default.to_string());
            }
            if field.optional {
                input = input.allow_empty(true);
            }
            input.interact_text()?
        };
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() && field.optional { continue; }
        if trimmed.is_empty() && field.default.is_none() && !field.optional {
            return Err(format!("{} is required", field.prompt).into());
        }
        values.push(FrontendConfigEntry { key: field.key.to_string(), value: trimmed, secret: field.secret });
    }
    Ok(values)
}
