//! [`SlackFrontend`] — actor-owned Slack messaging frontend.
//!
//! Replaces the old `client::{init,poll,send,shutdown}` free functions backed
//! by a `OnceLock<RwLock<SlackState>>`. The state lives on the struct, the
//! generic `FrontendActor<SlackFrontend>` wrapper turns it into a ractor
//! actor with a uniform message API.

use async_trait::async_trait;
use serde::Deserialize;

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

const COMPONENT: &str = "slack-frontend";
const SLACK_BOT_TOKEN_SYMBOLS: &[&str] = &["slack-bot-token", "slack-bot-token-v2"];
const SLACK_APP_TOKEN_SYMBOLS: &[&str] = &["slack-app-token", "slack-app-level-token"];

pub struct SlackFrontend {
    bot_token: String,
    app_token: String,
    last_ts: String,
    channels: Vec<String>,
}

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

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(sexp, key)
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

#[async_trait]
impl Frontend for SlackFrontend {
    type Config = String;

    fn name() -> &'static str {
        "slack"
    }

    async fn init(config: String) -> Result<Self, String> {
        if let Some(token) = extract_sexp_string(&config, "bot-token") {
            if !token.trim().is_empty() {
                harmonia_vault::set_secret_for_symbol("slack-bot-token", token.trim())?;
            }
        }
        if let Some(token) = extract_sexp_string(&config, "app-token") {
            if !token.trim().is_empty() {
                harmonia_vault::set_secret_for_symbol("slack-app-token", token.trim())?;
            }
        }

        let bot_token = read_vault_secret(SLACK_BOT_TOKEN_SYMBOLS)?.unwrap_or_default();
        let app_token = read_vault_secret(SLACK_APP_TOKEN_SYMBOLS)?.unwrap_or_default();
        let channels = extract_sexp_string_list(&config, "channels")
            .or_else(|| {
                harmonia_config_store::get_own(COMPONENT, "channels")
                    .ok()
                    .flatten()
                    .map(|v| parse_channels_csv(&v))
            })
            .unwrap_or_default();

        if bot_token.is_empty() {
            return Err("missing bot token in vault (symbol: slack-bot-token)".into());
        }
        if app_token.is_empty() {
            return Err("missing app token in vault (symbol: slack-app-token)".into());
        }
        if channels.is_empty() {
            return Err(
                "no channels configured: set (channels ...) or config-store slack-frontend/channels"
                    .into(),
            );
        }

        Ok(Self {
            bot_token,
            app_token,
            last_ts: String::from("0"),
            channels,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let mut results = Vec::new();
        let mut max_ts = self.last_ts.clone();

        for channel in &self.channels {
            let url = format!(
                "https://slack.com/api/conversations.history?channel={}&oldest={}",
                channel, self.last_ts
            );
            let resp = ureq::get(&url)
                .set("Authorization", &format!("Bearer {}", self.bot_token))
                .call()
                .map_err(|e| format!("slack api error for {channel}: {e}"))?;
            let body: SlackHistoryResponse = resp
                .into_json()
                .map_err(|e| format!("json parse error for {channel}: {e}"))?;

            if !body.ok {
                let err_msg = body.error.unwrap_or_else(|| "unknown error".into());
                return Err(format!("slack api error for {channel}: {err_msg}"));
            }

            for msg in &body.messages {
                if msg.subtype.is_some() {
                    continue;
                }
                if !msg.text.is_empty() {
                    let node_id = msg.user.as_deref().unwrap_or("unknown");
                    let metadata = format!(
                        "(:channel-class \"slack-bot\" :node-id \"{}\" :remote t)",
                        node_id.replace('\\', "\\\\").replace('"', "\\\"")
                    );
                    results.push(InboundMessage {
                        address: channel.clone(),
                        text: msg.text.clone(),
                        metadata: Some(metadata),
                    });
                }
                if msg.ts > max_ts {
                    max_ts = msg.ts.clone();
                }
            }
        }

        if max_ts != self.last_ts {
            self.last_ts = max_ts;
        }

        Ok(results)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let payload = serde_json::json!({ "channel": channel, "text": text });
        let resp = ureq::post("https://slack.com/api/chat.postMessage")
            .set("Authorization", &format!("Bearer {}", self.bot_token))
            .set("Content-Type", "application/json")
            .send_string(&payload.to_string())
            .map_err(|e| format!("slack send error: {e}"))?;
        let body: SlackPostResponse = resp
            .into_json()
            .map_err(|e| format!("json parse error: {e}"))?;
        if !body.ok {
            let err_msg = body.error.unwrap_or_else(|| "unknown error".into());
            return Err(format!("slack send failed: {err_msg}"));
        }
        Ok(())
    }

    async fn shutdown(&mut self) {
        // Nothing to release — bot/app tokens stay in vault, channel list is
        // pure data. The actor framework drops `self` when this returns.
        self.bot_token.clear();
        self.app_token.clear();
        self.channels.clear();
        self.last_ts = String::from("0");
    }
}
