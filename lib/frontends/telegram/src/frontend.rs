//! [`TelegramFrontend`] — actor-owned Telegram bot frontend.

use async_trait::async_trait;
use serde::Deserialize;

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

const COMPONENT: &str = "telegram-frontend";
const TELEGRAM_BOT_TOKEN_SYMBOLS: &[&str] = &["telegram-bot-token", "telegram-bot-api-token"];

#[derive(Debug, Deserialize)]
struct TgResponse {
    ok: bool,
    result: Option<Vec<TgUpdate>>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TgUpdate {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Debug, Deserialize)]
struct TgMessage {
    chat: TgChat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TgChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TgSendResponse {
    ok: bool,
    description: Option<String>,
}

pub struct TelegramFrontend {
    bot_token: String,
    last_update_id: i64,
}

fn sexp_value(config: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(config, key)
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
impl Frontend for TelegramFrontend {
    type Config = String;

    fn name() -> &'static str {
        "telegram"
    }

    async fn init(config: String) -> Result<Self, String> {
        if let Some(token) = sexp_value(&config, ":bot-token") {
            if !token.trim().is_empty() {
                harmonia_vault::set_secret_for_symbol("telegram-bot-token", token.trim())?;
            }
        }
        let bot_token = read_vault_secret(TELEGRAM_BOT_TOKEN_SYMBOLS)?.unwrap_or_default();
        if bot_token.is_empty() {
            return Err("no bot token provided in vault (symbol: telegram-bot-token)".into());
        }
        Ok(Self {
            bot_token,
            last_update_id: 0,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=1",
            self.bot_token,
            self.last_update_id + 1
        );
        let resp = ureq::get(&url).call().map_err(|e| format!("http: {e}"))?;
        let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
        let tg: TgResponse =
            serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
        if !tg.ok {
            return Err(format!(
                "telegram api error: {}",
                tg.description.unwrap_or_else(|| "unknown".into())
            ));
        }

        let updates = tg.result.unwrap_or_default();
        let mut results = Vec::new();
        let mut max_id = self.last_update_id;
        for u in &updates {
            if u.update_id > max_id {
                max_id = u.update_id;
            }
            if let Some(ref msg) = u.message {
                if let Some(ref text) = msg.text {
                    let chat_id = msg.chat.id.to_string();
                    let metadata = format!(
                        "(:channel-class \"telegram-bot\" :node-id \"{}\" :remote t)",
                        chat_id
                    );
                    results.push(InboundMessage {
                        address: chat_id,
                        text: text.clone(),
                        metadata: Some(metadata),
                    });
                }
            }
        }
        if max_id > self.last_update_id {
            self.last_update_id = max_id;
        }
        Ok(results)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.bot_token);
        let payload = serde_json::json!({ "chat_id": channel, "text": text });
        let resp = ureq::post(&url)
            .set("Content-Type", "application/json")
            .send_string(&payload.to_string())
            .map_err(|e| format!("http: {e}"))?;
        let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
        let tg: TgSendResponse =
            serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;
        if !tg.ok {
            return Err(format!(
                "sendMessage failed: {}",
                tg.description.unwrap_or_else(|| "unknown".into())
            ));
        }
        Ok(())
    }

    async fn shutdown(&mut self) {
        self.bot_token.clear();
        self.last_update_id = 0;
    }
}
