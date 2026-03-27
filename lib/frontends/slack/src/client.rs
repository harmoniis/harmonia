use serde::Deserialize;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "slack-frontend";
const SLACK_BOT_TOKEN_SYMBOLS: &[&str] = &["slack-bot-token", "slack-bot-token-v2"];
const SLACK_APP_TOKEN_SYMBOLS: &[&str] = &["slack-app-token", "slack-app-level-token"];

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct SlackState {
    pub bot_token: String,
    pub app_token: String,
    pub last_ts: String,
    pub channels: Vec<String>,
    pub initialized: bool,
}

/// Legacy singleton — deprecated. Frontend actor should own this state.
static LEGACY_STATE: OnceLock<RwLock<SlackState>> = OnceLock::new();

fn state() -> &'static RwLock<SlackState> {
    LEGACY_STATE.get_or_init(|| {
        RwLock::new(SlackState {
            bot_token: String::new(),
            app_token: String::new(),
            last_ts: String::from("0"),
            channels: Vec::new(),
            initialized: false,
        })
    })
}

// ---------------------------------------------------------------------------
// Slack API response types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SlackHistoryResponse {
    ok: bool,
    #[serde(default)]
    messages: Vec<SlackMessage>,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct SlackMessage {
    #[serde(default)]
    ts: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

#[derive(Deserialize)]
struct SlackPostResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

// ---------------------------------------------------------------------------
// Config parsing helpers
// ---------------------------------------------------------------------------

/// Extract a value for a key from a simple s-expression config.
/// Supports: (key "value") for strings, (key val1 val2) for lists.
fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pattern = format!("({} \"", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("({} ", key);
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

fn parse_channels_csv(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the Slack client from an s-expression config string.
///
/// Expected format (all fields optional, falls back to env vars):
/// ```text
/// (slack-config
///   (bot-token "xoxb-...")
///   (app-token "xapp-...")
///   (channels "C01ABC" "C02DEF"))
/// ```
pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {}", e))?;
    if s.initialized {
        return Err("slack already initialized".into());
    }

    if let Some(token) = extract_sexp_string(config, "bot-token") {
        if !token.trim().is_empty() {
            harmonia_vault::set_secret_for_symbol("slack-bot-token", token.trim())?;
        }
    }
    if let Some(token) = extract_sexp_string(config, "app-token") {
        if !token.trim().is_empty() {
            harmonia_vault::set_secret_for_symbol("slack-app-token", token.trim())?;
        }
    }

    s.bot_token = read_vault_secret(SLACK_BOT_TOKEN_SYMBOLS)?.unwrap_or_default();
    s.app_token = read_vault_secret(SLACK_APP_TOKEN_SYMBOLS)?.unwrap_or_default();

    s.channels = extract_sexp_string_list(config, "channels")
        .or_else(|| {
            harmonia_config_store::get_own(COMPONENT, "channels")
                .ok()
                .flatten()
                .map(|v| parse_channels_csv(&v))
        })
        .unwrap_or_default();

    if s.bot_token.is_empty() {
        return Err("missing bot token in vault (symbol: slack-bot-token)".into());
    }

    if s.app_token.is_empty() {
        return Err("missing app token in vault (symbol: slack-app-token)".into());
    }

    if s.channels.is_empty() {
        return Err(
            "no channels configured: set (channels ...) or config-store slack-frontend/channels"
                .into(),
        );
    }

    s.last_ts = String::from("0");
    s.initialized = true;

    Ok(())
}

/// Poll all monitored channels for new messages since the last timestamp.
///
/// Returns a list of (channel_id, text, metadata) triples for each new message.
pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (token, channels, oldest) = {
        let s = state().read().map_err(|e| format!("lock: {}", e))?;
        if !s.initialized {
            return Err("slack not initialized".into());
        }
        (s.bot_token.clone(), s.channels.clone(), s.last_ts.clone())
    };

    let mut results = Vec::new();
    let mut max_ts = oldest.clone();

    for channel in &channels {
        let url = format!(
            "https://slack.com/api/conversations.history?channel={}&oldest={}",
            channel, oldest
        );

        let resp = ureq::get(&url)
            .set("Authorization", &format!("Bearer {}", token))
            .call()
            .map_err(|e| format!("slack api error for {}: {}", channel, e))?;

        let body: SlackHistoryResponse = resp
            .into_json()
            .map_err(|e| format!("json parse error for {}: {}", channel, e))?;

        if !body.ok {
            let err_msg = body.error.unwrap_or_else(|| "unknown error".into());
            return Err(format!("slack api error for {}: {}", channel, err_msg));
        }

        for msg in &body.messages {
            // Skip bot messages and subtypes like channel_join, etc.
            if msg.subtype.is_some() {
                continue;
            }
            if !msg.text.is_empty() {
                let node_id = msg.user.as_deref().unwrap_or("unknown");
                let metadata = format!(
                    "(:channel-class \"slack-bot\" :node-id \"{}\" :remote t)",
                    node_id.replace('\\', "\\\\").replace('"', "\\\"")
                );
                results.push((channel.clone(), msg.text.clone(), Some(metadata)));
            }
            // Track the highest timestamp we've seen.
            if msg.ts > max_ts {
                max_ts = msg.ts.clone();
            }
        }
    }

    // Update last_ts so next poll only gets newer messages.
    if max_ts != oldest {
        if let Ok(mut s) = state().write() {
            s.last_ts = max_ts;
        }
    }

    Ok(results)
}

/// Send a text message to a Slack channel.
pub fn send(channel_id: &str, text: &str) -> Result<(), String> {
    let token = {
        let s = state().read().map_err(|e| format!("lock: {}", e))?;
        if !s.initialized {
            return Err("slack not initialized".into());
        }
        s.bot_token.clone()
    };

    let payload = serde_json::json!({
        "channel": channel_id,
        "text": text,
    });

    let resp = ureq::post("https://slack.com/api/chat.postMessage")
        .set("Authorization", &format!("Bearer {}", token))
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|e| format!("slack send error: {}", e))?;

    let body: SlackPostResponse = resp
        .into_json()
        .map_err(|e| format!("json parse error: {}", e))?;

    if !body.ok {
        let err_msg = body.error.unwrap_or_else(|| "unknown error".into());
        return Err(format!("slack send failed: {}", err_msg));
    }

    Ok(())
}

/// Shut down the Slack client and clear state.
pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.bot_token.clear();
        s.app_token.clear();
        s.channels.clear();
        s.last_ts = String::from("0");
        s.initialized = false;
    }
}
