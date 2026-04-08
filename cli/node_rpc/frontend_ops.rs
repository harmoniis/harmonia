//! Frontend pairing, configuration, and status operations.

use harmonia_node_rpc::NodeRpcResult;
use std::path::Path;

use super::helpers::{bind_vault_env, config_has, config_set, vault_has};
use super::frontend_verify;

// --- Public RPC entry points ---

pub(crate) fn frontend_pair_list() -> Result<NodeRpcResult, String> {
    let frontends = list_pairable_frontends();
    Ok(NodeRpcResult::FrontendPairList { frontends })
}

pub(crate) fn frontend_configure_rpc(
    frontend: &str,
    values: &[harmonia_node_rpc::FrontendConfigEntry],
) -> Result<NodeRpcResult, String> {
    let (qr_data, instructions) = frontend_configure(frontend, values)?;
    Ok(NodeRpcResult::FrontendConfigure {
        frontend: frontend.to_string(),
        qr_data,
        instructions,
    })
}

pub(crate) fn frontend_pair_init_rpc(frontend: &str) -> Result<NodeRpcResult, String> {
    let (qr_data, instructions) = frontend_pair_init(frontend)?;
    Ok(NodeRpcResult::FrontendPairInit {
        frontend: frontend.to_string(),
        qr_data,
        instructions,
    })
}

pub(crate) fn frontend_pair_status_rpc(frontend: &str) -> Result<NodeRpcResult, String> {
    let (paired, message) = frontend_pair_status(frontend)?;
    Ok(NodeRpcResult::FrontendPairStatus {
        frontend: frontend.to_string(),
        paired,
        message,
    })
}

// --- Public local wrappers ---

pub fn list_pairable_frontends_local() -> Vec<harmonia_node_rpc::PairableFrontend> {
    list_pairable_frontends()
}

pub fn frontend_pair_init_local(frontend: &str) -> Result<(Option<String>, String), String> {
    frontend_pair_init(frontend)
}

pub fn frontend_pair_status_local(frontend: &str) -> Result<(bool, String), String> {
    frontend_pair_status(frontend)
}

pub fn frontend_configure_local(
    frontend: &str,
    values: &[harmonia_node_rpc::FrontendConfigEntry],
) -> Result<(Option<String>, String), String> {
    frontend_configure(frontend, values)
}

// --- Helpers ---

fn set_secret_if_present(value: Option<&str>, symbols: &[&str]) -> Result<(), String> {
    if let Some(value) = value {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let symbol = symbols.first().ok_or("missing vault symbol")?;
            harmonia_vault::set_secret_for_symbol(symbol, trimmed)?;
        }
    }
    Ok(())
}

fn value_for<'a>(
    values: &'a [harmonia_node_rpc::FrontendConfigEntry],
    key: &str,
) -> Option<&'a str> {
    values
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.value.as_str())
}

fn default_if_empty(value: Option<&str>, default: &'static str) -> String {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
        .to_string()
}

fn frontend_configure(
    frontend: &str,
    values: &[harmonia_node_rpc::FrontendConfigEntry],
) -> Result<(Option<String>, String), String> {
    bind_vault_env()?;
    let _ = harmonia_vault::init_from_env();

    match frontend {
        "telegram" => {
            set_secret_if_present(value_for(values, "bot-token"), &["telegram-bot-token"])?;
            let ok = frontend_verify::verify_telegram_token()?;
            if !ok { return Err("Telegram bot token is invalid".to_string()); }
            Ok((None, "Telegram bot token verified. Frontend ready.".to_string()))
        }
        "slack" => {
            set_secret_if_present(value_for(values, "bot-token"), &["slack-bot-token"])?;
            set_secret_if_present(value_for(values, "app-token"), &["slack-app-token"])?;
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if channels.is_empty() { return Err("Slack requires at least one channel ID".to_string()); }
            config_set("slack-frontend", "channels", channels)?;
            let ok = frontend_verify::verify_slack_token()?;
            if !ok { return Err("Slack token verification failed".to_string()); }
            Ok((None, format!("Slack verified. Saved channels: {channels}")))
        }
        "discord" => {
            set_secret_if_present(value_for(values, "bot-token"), &["discord-bot-token"])?;
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if channels.is_empty() { return Err("Discord requires at least one channel ID".to_string()); }
            config_set("discord-frontend", "channels", channels)?;
            let ok = frontend_verify::verify_discord_token()?;
            if !ok { return Err("Discord token verification failed".to_string()); }
            Ok((None, format!("Discord verified. Saved channels: {channels}")))
        }
        "mattermost" => {
            let api_url = value_for(values, "api-url").unwrap_or("").trim();
            let channels = value_for(values, "channels").unwrap_or("").trim();
            if api_url.is_empty() { return Err("Mattermost requires api-url".to_string()); }
            if channels.is_empty() { return Err("Mattermost requires at least one channel ID".to_string()); }
            config_set("mattermost-frontend", "api-url", api_url)?;
            config_set("mattermost-frontend", "channels", channels)?;
            set_secret_if_present(value_for(values, "bot-token"), &["mattermost-bot-token"])?;
            let ok = frontend_verify::verify_mattermost_token()?;
            if !ok { return Err("Mattermost auth failed".to_string()); }
            Ok((None, format!("Mattermost verified. Saved channels: {channels}")))
        }
        "email" => configure_email(values),
        "nostr" => configure_nostr(values),
        "whatsapp" => {
            let api_url = default_if_empty(value_for(values, "api-url"), "http://127.0.0.1:3000");
            config_set("whatsapp-frontend", "api-url", &api_url)?;
            set_secret_if_present(value_for(values, "api-key"), &["whatsapp-api-key"])?;
            frontend_pair_init(frontend)
        }
        "signal" => {
            let rpc_url = default_if_empty(value_for(values, "rpc-url"), "http://127.0.0.1:8080");
            config_set("signal-frontend", "rpc-url", &rpc_url)?;
            set_secret_if_present(value_for(values, "auth-token"), &["signal-auth-token"])?;
            frontend_pair_init(frontend)
        }
        #[cfg(target_os = "macos")]
        "imessage" => {
            let server_url = value_for(values, "server-url").unwrap_or("").trim();
            if server_url.is_empty() { return Err("iMessage requires server-url".to_string()); }
            config_set("imessage-frontend", "server-url", server_url)?;
            set_secret_if_present(value_for(values, "password"), &["bluebubbles-password"])?;
            let ok = frontend_verify::verify_imessage_bridge()?;
            if !ok { return Err("BlueBubbles bridge is not responding".to_string()); }
            Ok((None, "BlueBubbles bridge verified. Frontend ready.".to_string()))
        }
        "tailscale" => {
            set_secret_if_present(value_for(values, "auth-key"), &["tailscale-auth-key"])?;
            Ok((None, "Tailscale auth key saved.".to_string()))
        }
        "http2" => configure_http2(values),
        "mqtt" => Ok((None, "MQTT is managed automatically by Harmonia.".to_string())),
        _ => Err(format!("frontend '{frontend}' does not support configuration")),
    }
}

fn configure_email(values: &[harmonia_node_rpc::FrontendConfigEntry]) -> Result<(Option<String>, String), String> {
    let required = ["imap-host","imap-port","imap-user","imap-mailbox","imap-tls","smtp-host","smtp-port","smtp-user","smtp-from","smtp-tls"];
    for key in required {
        let value = value_for(values, key).unwrap_or("").trim();
        if value.is_empty() { return Err(format!("Email requires {key}")); }
        config_set("email-frontend", key, value)?;
    }
    set_secret_if_present(value_for(values, "imap-password"), &["email-imap-password"])?;
    set_secret_if_present(value_for(values, "smtp-password"), &["email-smtp-password"])?;
    if !vault_has("email-frontend", &["email-imap-password","email-password","email-smtp-password"]) {
        return Err("Email requires an IMAP password".to_string());
    }
    Ok((None, "Email settings saved.".to_string()))
}

fn configure_nostr(values: &[harmonia_node_rpc::FrontendConfigEntry]) -> Result<(Option<String>, String), String> {
    let key = value_for(values, "private-key").unwrap_or("").trim();
    if key.is_empty() { return Err("Nostr requires a private key".to_string()); }
    harmonia_vault::set_secret_for_symbol("nostr-private-key", key)?;
    let relays = default_if_empty(value_for(values, "relays"), "wss://relay.damus.io,wss://relay.primal.net,wss://nos.lol");
    config_set("nostr-frontend", "relays", &relays)?;
    Ok((None, format!("Nostr configured with relays: {relays}")))
}

fn configure_http2(values: &[harmonia_node_rpc::FrontendConfigEntry]) -> Result<(Option<String>, String), String> {
    let bind = default_if_empty(value_for(values, "bind"), "127.0.0.1:9443");
    bind.parse::<std::net::SocketAddr>().map_err(|e| format!("invalid bind address: {e}"))?;
    let ca_cert = value_for(values, "ca-cert").unwrap_or("").trim();
    let server_cert = value_for(values, "server-cert").unwrap_or("").trim();
    let server_key = value_for(values, "server-key").unwrap_or("").trim();
    for (label, path) in [("ca-cert", ca_cert), ("server-cert", server_cert), ("server-key", server_key)] {
        if path.is_empty() { return Err(format!("HTTP/2 requires {label}")); }
        if !Path::new(path).exists() { return Err(format!("HTTP/2 {label} path does not exist: {path}")); }
    }
    let trusted_csv = value_for(values, "trusted-client-fingerprints").unwrap_or("").trim().to_string();
    if trusted_csv.is_empty() { return Err("HTTP/2 requires at least one trusted client fingerprint".to_string()); }
    let trusted: Vec<String> = trusted_csv.split(',').map(harmonia_transport_auth::normalize_fingerprint).filter(|value| !value.is_empty()).collect();
    if trusted.is_empty() { return Err("HTTP/2 requires at least one valid trusted client fingerprint".to_string()); }
    config_set("http2-frontend", "bind", &bind)?;
    config_set("http2-frontend", "ca-cert", ca_cert)?;
    config_set("http2-frontend", "server-cert", server_cert)?;
    config_set("http2-frontend", "server-key", server_key)?;
    config_set("http2-frontend", "trusted-client-fingerprints-json", &serde_json::to_string(&trusted).map_err(|e| e.to_string())?)?;
    for key in ["max-concurrent-streams","session-idle-timeout-ms","max-frame-bytes"] {
        let value = value_for(values, key).unwrap_or("").trim();
        if value.is_empty() { continue; }
        config_set("http2-frontend", key, value)?;
    }
    Ok((None, format!("HTTP/2 mTLS configured on {bind}. Trusted client identities: {}", trusted.join(", "))))
}

pub(crate) fn list_pairable_frontends() -> Vec<harmonia_node_rpc::PairableFrontend> {
    let _ = harmonia_vault::init_from_env();
    let mut frontends = Vec::new();

    // MQTT
    frontends.push(harmonia_node_rpc::PairableFrontend {
        name: "mqtt".into(), display: "MQTT".into(),
        status: if config_has("mqtt-frontend", "broker") { "configured".into() } else { "not configured".into() },
        pairable: false,
    });

    // WhatsApp
    {
        let configured = config_has("whatsapp-frontend", "api-url");
        let (status, pairable) = if configured {
            match harmonia_whatsapp::client::pair_status() {
                Ok((true, _)) => ("connected".into(), false),
                Ok((false, msg)) => (msg, true),
                Err(_) => ("bridge unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "whatsapp".into(), display: "WhatsApp".into(), status, pairable });
    }

    // Signal
    {
        let configured = config_has("signal-frontend", "rpc-url") || config_has("signal-frontend", "account");
        let (status, pairable) = if configured {
            match harmonia_signal::client::pair_status() {
                Ok((true, _)) => ("device linked".into(), false),
                Ok((false, msg)) => (msg, true),
                Err(_) => ("bridge unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "signal".into(), display: "Signal".into(), status, pairable });
    }

    frontend_verify::append_token_frontends(&mut frontends);

    frontends
}

fn frontend_pair_init(frontend: &str) -> Result<(Option<String>, String), String> {
    let _ = harmonia_vault::init_from_env();
    match frontend {
        "whatsapp" => {
            let qr = harmonia_whatsapp::client::pair_init()?;
            Ok((qr, "Scan the QR code with WhatsApp on your phone:\nWhatsApp > Settings > Linked Devices > Link a Device".to_string()))
        }
        "signal" => {
            let uri = harmonia_signal::client::pair_init()?;
            Ok((uri, "Scan the QR code with Signal on your phone:\nSignal > Settings > Linked Devices > Link New Device".to_string()))
        }
        other => frontend_verify::pair_init_token_based(other),
    }
}

fn frontend_pair_status(frontend: &str) -> Result<(bool, String), String> {
    let _ = harmonia_vault::init_from_env();
    match frontend {
        "whatsapp" => harmonia_whatsapp::client::pair_status(),
        "signal" => {
            let status = harmonia_signal::client::pair_status()?;
            if status.0 { let _ = frontend_verify::discover_and_store_signal_account(); }
            Ok(status)
        }
        other => frontend_verify::pair_status_token_based(other),
    }
}
