//! Frontend token verification and status helpers.

use super::helpers::{config_get, config_has, config_set, vault_get, vault_has};

// --- HTTP verification helpers ---

fn http_ok(url: &str, bearer: Option<&str>) -> Result<bool, String> {
    let req = ureq::get(url);
    let req = match bearer {
        Some(token) => req.set("Authorization", &format!("Bearer {token}")),
        None => req,
    };
    match req.call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(code, _)) => Ok(code < 500),
        Err(e) => Err(format!("{e}")),
    }
}

fn http_ok_bot(url: &str, token: &str) -> Result<bool, String> {
    let req = ureq::get(url)
        .set("Authorization", &format!("Bot {token}"))
        .set("User-Agent", "harmonia-discord/0.1.0");
    match req.call() {
        Ok(_) => Ok(true),
        Err(ureq::Error::Status(401, _)) => Ok(false),
        Err(ureq::Error::Status(code, _)) => Ok(code < 500),
        Err(e) => Err(format!("{e}")),
    }
}

// --- Per-frontend token verification ---

pub(crate) fn verify_telegram_token() -> Result<bool, String> {
    let token = vault_get("telegram-frontend", &["telegram-bot-token", "telegram-bot-api-token"])
        .ok_or("no token")?;
    let url = format!("https://api.telegram.org/bot{token}/getMe");
    match ureq::get(&url).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            Ok(body.contains("\"ok\":true"))
        }
        Err(ureq::Error::Status(401, _)) => Ok(false),
        Err(e) => Err(format!("{e}")),
    }
}

pub(crate) fn verify_slack_token() -> Result<bool, String> {
    let token = vault_get("slack-frontend", &["slack-bot-token", "slack-bot-token-v2"])
        .ok_or("no token")?;
    match ureq::post("https://slack.com/api/auth.test")
        .set("Authorization", &format!("Bearer {token}"))
        .set("Content-Type", "application/x-www-form-urlencoded")
        .send_string("")
    {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            Ok(body.contains("\"ok\":true"))
        }
        Err(e) => Err(format!("{e}")),
    }
}

pub(crate) fn verify_discord_token() -> Result<bool, String> {
    let token = vault_get("discord-frontend", &["discord-bot-token", "discord-token"])
        .ok_or("no token")?;
    http_ok_bot("https://discord.com/api/v10/users/@me", &token)
}

pub(crate) fn verify_mattermost_token() -> Result<bool, String> {
    let url = config_get("mattermost-frontend", "api-url").ok_or("no url")?;
    let token = vault_get(
        "mattermost-frontend",
        &["mattermost-bot-token", "mattermost-token"],
    )
    .ok_or("no token")?;
    http_ok(&format!("{url}/api/v4/users/me"), Some(&token))
}

#[cfg(target_os = "macos")]
pub(crate) fn verify_imessage_bridge() -> Result<bool, String> {
    let url = config_get("imessage-frontend", "server-url").ok_or("no url")?;
    let password = vault_get(
        "imessage-frontend",
        &["bluebubbles-password", "imessage-password"],
    );
    http_ok(&format!("{url}/api/v1/server/info"), password.as_deref())
}

// --- Append token-based frontends to the pairable list ---

pub(crate) fn append_token_frontends(frontends: &mut Vec<harmonia_node_rpc::PairableFrontend>) {
    // Telegram
    {
        let has_token = vault_has("telegram-frontend", &["telegram-bot-token", "telegram-bot-api-token"]);
        let (status, pairable) = if has_token {
            match verify_telegram_token() {
                Ok(true) => ("connected".into(), false),
                Ok(false) => ("token invalid".into(), true),
                Err(_) => ("api unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "telegram".into(), display: "Telegram".into(), status, pairable });
    }
    // Slack
    {
        let has_token = vault_has("slack-frontend", &["slack-bot-token", "slack-bot-token-v2"]);
        let (status, pairable) = if has_token {
            match verify_slack_token() {
                Ok(true) => ("connected".into(), false),
                Ok(false) => ("token invalid".into(), true),
                Err(_) => ("api unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "slack".into(), display: "Slack".into(), status, pairable });
    }
    // Discord
    {
        let has_token = vault_has("discord-frontend", &["discord-bot-token", "discord-token"]);
        let (status, pairable) = if has_token {
            match verify_discord_token() {
                Ok(true) => ("connected".into(), false),
                Ok(false) => ("token invalid".into(), true),
                Err(_) => ("api unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "discord".into(), display: "Discord".into(), status, pairable });
    }
    // Mattermost
    {
        let has_url = config_has("mattermost-frontend", "api-url");
        let has_token = vault_has("mattermost-frontend", &["mattermost-bot-token", "mattermost-token"]);
        let (status, pairable) = if has_url && has_token {
            match verify_mattermost_token() {
                Ok(true) => ("connected".into(), false),
                Ok(false) => ("auth failed".into(), true),
                Err(_) => ("server unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "mattermost".into(), display: "Mattermost".into(), status, pairable });
    }
    // Email
    {
        let has_host = config_has("email-frontend", "imap-host");
        let has_password = vault_has("email-frontend", &["email-imap-password","email-password","email-smtp-password"]);
        let (status, pairable) = if has_host && has_password { ("configured".into(), false) } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "email".into(), display: "Email".into(), status, pairable });
    }
    // iMessage (macOS)
    #[cfg(target_os = "macos")]
    {
        let has_url = config_has("imessage-frontend", "server-url");
        let (status, pairable) = if has_url {
            match verify_imessage_bridge() {
                Ok(true) => ("connected".into(), false),
                Ok(false) | Err(_) => ("bridge unreachable".into(), true),
            }
        } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "imessage".into(), display: "iMessage".into(), status, pairable });
    }
    // Nostr
    {
        let has_key = vault_has("nostr-frontend", &["nostr-private-key", "nostr-nsec"]);
        let (status, pairable) = if has_key { ("key configured".into(), false) } else { ("not configured".into(), false) };
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "nostr".into(), display: "Nostr".into(), status, pairable });
    }
    // Tailscale
    {
        let configured = vault_has("tailscale-frontend", &["tailscale-auth-key"]);
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "tailscale".into(), display: "Tailscale".into(), status: if configured { "configured".into() } else { "not configured".into() }, pairable: false });
    }
    // HTTP/2 mTLS
    {
        let configured = config_has("http2-frontend", "bind") && config_has("http2-frontend", "ca-cert") && config_has("http2-frontend", "server-cert") && config_has("http2-frontend", "server-key") && config_has("http2-frontend", "trusted-client-fingerprints-json");
        frontends.push(harmonia_node_rpc::PairableFrontend { name: "http2".into(), display: "HTTP/2 mTLS".into(), status: if configured { "configured".into() } else { "not configured".into() }, pairable: false });
    }
}

// --- Token-based pair init / status ---

pub(crate) fn pair_init_token_based(frontend: &str) -> Result<(Option<String>, String), String> {
    match frontend {
        "telegram" => match verify_telegram_token() {
            Ok(true) => Ok((None, "Telegram bot token verified. Bot is connected and receiving messages.".into())),
            Ok(false) => Err("Telegram bot token is invalid. Update it via `harmonia setup`.".into()),
            Err(e) => Err(format!("Cannot reach Telegram API: {e}")),
        },
        "slack" => match verify_slack_token() {
            Ok(true) => Ok((None, "Slack bot token verified. Bot is connected.".into())),
            Ok(false) => Err("Slack bot token is invalid. Update it via `harmonia setup`.".into()),
            Err(e) => Err(format!("Cannot reach Slack API: {e}")),
        },
        "discord" => match verify_discord_token() {
            Ok(true) => Ok((None, "Discord bot token verified. Bot is connected.".into())),
            Ok(false) => Err("Discord bot token is invalid. Update it via `harmonia setup`.".into()),
            Err(e) => Err(format!("Cannot reach Discord API: {e}")),
        },
        "mattermost" => match verify_mattermost_token() {
            Ok(true) => Ok((None, "Mattermost bot token verified. Bot is connected.".into())),
            Ok(false) => Err("Mattermost auth failed. Check api-url and bot token via `harmonia setup`.".into()),
            Err(e) => Err(format!("Cannot reach Mattermost server: {e}")),
        },
        #[cfg(target_os = "macos")]
        "imessage" => match verify_imessage_bridge() {
            Ok(true) => Ok((None, "BlueBubbles bridge is reachable. iMessage is connected.".into())),
            Ok(false) => Err("BlueBubbles bridge is not responding. Check server-url via `harmonia setup`.".into()),
            Err(e) => Err(format!("Cannot reach BlueBubbles: {e}")),
        },
        _ => Err(format!("frontend '{frontend}' does not support linking")),
    }
}

pub(crate) fn pair_status_token_based(frontend: &str) -> Result<(bool, String), String> {
    match frontend {
        "telegram" => verify_telegram_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.into())),
        "slack" => verify_slack_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.into())),
        "discord" => verify_discord_token().map(|ok| (ok, if ok { "connected" } else { "token invalid" }.into())),
        "mattermost" => verify_mattermost_token().map(|ok| (ok, if ok { "connected" } else { "auth failed" }.into())),
        #[cfg(target_os = "macos")]
        "imessage" => verify_imessage_bridge().map(|ok| (ok, if ok { "connected" } else { "bridge unreachable" }.into())),
        "email" => {
            let configured = config_has("email-frontend", "imap-host") && vault_has("email-frontend", &["email-imap-password","email-password"]);
            Ok((configured, if configured { "configured" } else { "not configured" }.into()))
        }
        "nostr" => {
            let configured = vault_has("nostr-frontend", &["nostr-private-key", "nostr-nsec"]);
            Ok((configured, if configured { "key configured" } else { "not configured" }.into()))
        }
        "http2" => {
            let configured = config_has("http2-frontend", "bind") && config_has("http2-frontend", "ca-cert") && config_has("http2-frontend", "server-cert") && config_has("http2-frontend", "server-key") && config_has("http2-frontend", "trusted-client-fingerprints-json");
            Ok((configured, if configured { "configured" } else { "not configured" }.into()))
        }
        _ => Err(format!("unknown frontend '{frontend}'")),
    }
}

// --- Signal account discovery ---

pub(crate) fn discover_and_store_signal_account() -> Result<(), String> {
    let rpc_url = config_get("signal-frontend", "rpc-url").unwrap_or_default();
    if rpc_url.trim().is_empty() || config_has("signal-frontend", "account") {
        return Ok(());
    }
    let auth = vault_get("signal-frontend", &["signal-auth-token", "signal-auth-token-v2"]);
    let endpoints = [format!("{rpc_url}/v1/accounts"), format!("{rpc_url}/v2/accounts")];
    for endpoint in endpoints {
        let req = ureq::get(&endpoint);
        let req = match auth.as_deref() {
            Some(token) => req.set("Authorization", &format!("Bearer {token}")),
            None => req,
        };
        let response = match req.call() {
            Ok(response) => response,
            Err(ureq::Error::Status(404, _)) | Err(_) => continue,
        };
        let json: serde_json::Value = match response.into_json() {
            Ok(json) => json,
            Err(_) => continue,
        };
        if let Some(account) = extract_signal_account(&json) {
            let _ = config_set("signal-frontend", "account", &account);
            return Ok(());
        }
    }
    Ok(())
}

fn extract_signal_account(json: &serde_json::Value) -> Option<String> {
    if let Some(account) = json.get("number").and_then(|value| value.as_str()) {
        let trimmed = account.trim();
        if !trimmed.is_empty() { return Some(trimmed.to_string()); }
    }
    if let Some(account) = json.get("account").and_then(|value| value.as_str()) {
        let trimmed = account.trim();
        if !trimmed.is_empty() { return Some(trimmed.to_string()); }
    }
    if let Some(accounts) = json.as_array() {
        for entry in accounts {
            if let Some(account) = extract_signal_account(entry) { return Some(account); }
        }
    }
    if let Some(results) = json.get("accounts").and_then(|value| value.as_array()) {
        for entry in results {
            if let Some(account) = extract_signal_account(entry) { return Some(account); }
        }
    }
    None
}
