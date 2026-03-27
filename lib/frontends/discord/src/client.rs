use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "discord-frontend";
const DISCORD_BOT_TOKEN_SYMBOLS: &[&str] = &["discord-bot-token", "discord-token"];

pub struct DiscordState {
    pub bot_token: String,
    pub channels: Vec<String>,
    pub last_message_ids: HashMap<String, String>,
    pub initialized: bool,
}

/// Legacy singleton — deprecated. Frontend actor should own this state.
static LEGACY_STATE: OnceLock<RwLock<DiscordState>> = OnceLock::new();

fn state() -> &'static RwLock<DiscordState> {
    LEGACY_STATE.get_or_init(|| {
        RwLock::new(DiscordState {
            bot_token: String::new(),
            channels: Vec::new(),
            last_message_ids: HashMap::new(),
            initialized: false,
        })
    })
}

#[derive(Deserialize)]
struct DiscordUser {
    #[serde(default)]
    id: String,
    #[serde(default)]
    bot: bool,
}

#[derive(Deserialize)]
struct DiscordMessage {
    #[serde(default)]
    id: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    author: Option<DiscordUser>,
}

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(sexp, key)
}

fn extract_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("({key} ");
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find(')')?;
    let segment = &rest[..end];
    let items: Vec<String> = segment
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.to_string())
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

fn parse_channels_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env()?;
    for symbol in symbols {
        let maybe = harmonia_vault::get_secret_for_component(COMPONENT, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(value) = maybe {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }
    Ok(None)
}

fn parse_snowflake(id: &str) -> u64 {
    id.parse::<u64>().unwrap_or(0)
}

fn is_bot_message(message: &DiscordMessage) -> bool {
    message.author.as_ref().map(|u| u.bot).unwrap_or(false)
}

fn request_json<T: DeserializeOwned>(req: ureq::Request) -> Result<T, String> {
    req.call()
        .map_err(format_request_error)?
        .into_json()
        .map_err(|e| format!("discord json decode failed: {e}"))
}

fn format_request_error(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            if body.is_empty() {
                format!("discord api status {code}")
            } else {
                format!("discord api status {code}: {body}")
            }
        }
        ureq::Error::Transport(t) => format!("discord transport error: {t}"),
    }
}

fn list_messages(
    token: &str,
    channel: &str,
    after_id: Option<&str>,
) -> Result<Vec<DiscordMessage>, String> {
    let mut url = format!("https://discord.com/api/v10/channels/{channel}/messages?limit=50");
    if let Some(after) = after_id {
        if !after.is_empty() {
            url.push_str("&after=");
            url.push_str(after);
        }
    }
    request_json(
        ureq::get(&url)
            .set("Authorization", &format!("Bot {token}"))
            .set("User-Agent", "harmonia-discord/0.1.0"),
    )
}

pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;
    if s.initialized {
        return Err("discord already initialized".into());
    }

    if let Some(token) = extract_sexp_string(config, "bot-token") {
        if !token.trim().is_empty() {
            harmonia_vault::set_secret_for_symbol("discord-bot-token", token.trim())?;
        }
    }

    s.bot_token = read_vault_secret(DISCORD_BOT_TOKEN_SYMBOLS)?.unwrap_or_default();

    s.channels = extract_sexp_string_list(config, "channels")
        .or_else(|| {
            harmonia_config_store::get_own(COMPONENT, "channels")
                .ok()
                .flatten()
                .map(|v| parse_channels_csv(&v))
        })
        .unwrap_or_default();

    if s.bot_token.is_empty() {
        return Err("missing bot token in vault (symbol: discord-bot-token)".into());
    }
    if s.channels.is_empty() {
        return Err(
            "missing channels: set (channels ...) or config-store discord-frontend/channels".into(),
        );
    }

    s.last_message_ids = s
        .channels
        .iter()
        .map(|c| (c.clone(), String::new()))
        .collect();
    s.initialized = true;
    Ok(())
}

pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (token, channels, cursors) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("discord not initialized".into());
        }
        (
            s.bot_token.clone(),
            s.channels.clone(),
            s.last_message_ids.clone(),
        )
    };

    let mut next_cursors = cursors;
    let mut outbound = Vec::new();

    for channel in &channels {
        let cursor = next_cursors.get(channel).cloned().unwrap_or_default();
        let mut messages = if cursor.is_empty() {
            // First poll only advances the cursor to avoid replaying channel history.
            list_messages(&token, channel, None)?
                .into_iter()
                .take(1)
                .collect::<Vec<_>>()
        } else {
            list_messages(&token, channel, Some(&cursor))?
        };

        if messages.is_empty() {
            continue;
        }

        messages.sort_by_key(|m| parse_snowflake(&m.id));

        let previous = parse_snowflake(&cursor);
        let mut max_seen = previous;
        let initial_sync = cursor.is_empty();

        for msg in messages {
            let current_id = parse_snowflake(&msg.id);
            if current_id > max_seen {
                max_seen = current_id;
            }
            if initial_sync {
                // Initial sync establishes cursor only; no historical emit.
                continue;
            }
            if is_bot_message(&msg) || msg.content.trim().is_empty() {
                continue;
            }
            let author_id = msg
                .author
                .as_ref()
                .map(|u| u.id.as_str())
                .unwrap_or("unknown");
            let metadata = format!(
                "(:channel-class \"discord-bot\" :node-id \"{}\" :remote t)",
                author_id.replace('\\', "\\\\").replace('"', "\\\"")
            );
            outbound.push((channel.clone(), msg.content, Some(metadata)));
        }

        if max_seen > previous {
            next_cursors.insert(channel.clone(), max_seen.to_string());
        }
    }

    if let Ok(mut s) = state().write() {
        s.last_message_ids = next_cursors;
    }

    Ok(outbound)
}

pub fn send(channel_id: &str, text: &str) -> Result<(), String> {
    let token = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("discord not initialized".into());
        }
        s.bot_token.clone()
    };

    let url = format!("https://discord.com/api/v10/channels/{channel_id}/messages");
    let payload = serde_json::json!({
        "content": text,
    });

    ureq::post(&url)
        .set("Authorization", &format!("Bot {token}"))
        .set("User-Agent", "harmonia-discord/0.1.0")
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(format_request_error)?;

    Ok(())
}

pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.bot_token.clear();
        s.channels.clear();
        s.last_message_ids.clear();
        s.initialized = false;
    }
}
