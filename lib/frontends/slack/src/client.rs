use serde::Deserialize;
use std::sync::{OnceLock, RwLock};

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

static STATE: OnceLock<RwLock<SlackState>> = OnceLock::new();

fn state() -> &'static RwLock<SlackState> {
    STATE.get_or_init(|| {
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
///
/// Environment variable fallbacks:
///   HARMONIA_SLACK_BOT_TOKEN, HARMONIA_SLACK_APP_TOKEN, HARMONIA_SLACK_CHANNELS
pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {}", e))?;
    if s.initialized {
        return Err("slack already initialized".into());
    }

    // Bot token: config -> env
    s.bot_token = extract_sexp_string(config, "bot-token")
        .or_else(|| std::env::var("HARMONIA_SLACK_BOT_TOKEN").ok())
        .unwrap_or_default();

    // App token: config -> env
    s.app_token = extract_sexp_string(config, "app-token")
        .or_else(|| std::env::var("HARMONIA_SLACK_APP_TOKEN").ok())
        .unwrap_or_default();

    // Channels: config -> env (comma-separated)
    s.channels = extract_sexp_string_list(config, "channels")
        .or_else(|| {
            std::env::var("HARMONIA_SLACK_CHANNELS")
                .ok()
                .map(|v| v.split(',').map(|c| c.trim().to_string()).collect())
        })
        .unwrap_or_default();

    if s.bot_token.is_empty() {
        return Err(
            "missing bot token: set (bot-token ...) in config or HARMONIA_SLACK_BOT_TOKEN env"
                .into(),
        );
    }

    if s.channels.is_empty() {
        return Err(
            "no channels configured: set (channels ...) in config or HARMONIA_SLACK_CHANNELS env"
                .into(),
        );
    }

    s.last_ts = String::from("0");
    s.initialized = true;

    Ok(())
}

/// Poll all monitored channels for new messages since the last timestamp.
///
/// Returns a list of (channel_id, text) pairs for each new message.
pub fn poll() -> Result<Vec<(String, String)>, String> {
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
                results.push((channel.clone(), msg.text.clone()));
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
