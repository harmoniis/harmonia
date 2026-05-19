//! [`WhatsAppFrontend`] — actor-owned WhatsApp bridge frontend.
//!
//! State for the running poll/send loop lives on `Self`. The pairing helpers
//! (`pair_init`, `pair_status`) are state-free and live in `pairing` — they
//! read config-store / vault directly, which is what the CLI side needs.

use async_trait::async_trait;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

pub(crate) const COMPONENT: &str = "whatsapp-frontend";
pub(crate) const WHATSAPP_API_KEY_SYMBOLS: &[&str] = &["whatsapp-session", "whatsapp-api-key"];
pub(crate) const WHATSAPP_API_URL_SYMBOLS: &[&str] = &["whatsapp-bridge-url", "whatsapp-api-url"];

#[derive(Debug, Deserialize)]
struct WaMessage {
    from: String,
    body: String,
}

pub struct WhatsAppFrontend {
    api_url: String,
    api_key: String,
    last_poll_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn sexp_value(config: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(config, key)
}

pub(crate) fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
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
impl Frontend for WhatsAppFrontend {
    type Config = String;

    fn name() -> &'static str {
        "whatsapp"
    }

    async fn init(config: String) -> Result<Self, String> {
        if let Some(api_url) = sexp_value(&config, ":api-url") {
            let trimmed = api_url.trim();
            if !trimmed.is_empty() {
                let _ =
                    harmonia_config_store::set_config(COMPONENT, COMPONENT, "api-url", trimmed);
            }
        }
        if let Some(api_key) = sexp_value(&config, ":api-key") {
            let trimmed = api_key.trim();
            if !trimmed.is_empty() {
                harmonia_vault::set_secret_for_symbol("whatsapp-session", trimmed)?;
            }
        }

        let mut api_url = harmonia_config_store::get_own(COMPONENT, "api-url")
            .ok()
            .flatten()
            .or_else(|| {
                read_vault_secret(WHATSAPP_API_URL_SYMBOLS)
                    .ok()
                    .flatten()
                    .map(|legacy| {
                        let _ = harmonia_config_store::set_config(
                            COMPONENT, COMPONENT, "api-url", &legacy,
                        );
                        legacy
                    })
            })
            .unwrap_or_else(|| "http://127.0.0.1:3000".into());
        if api_url.ends_with('/') {
            api_url.pop();
        }
        let api_key = read_vault_secret(WHATSAPP_API_KEY_SYMBOLS)?.unwrap_or_default();

        Ok(Self {
            api_url,
            api_key,
            last_poll_ms: now_ms(),
        })
    }

    async fn poll(&mut self) -> PollResult {
        let endpoint = format!("{}/api/messages?since={}", self.api_url, self.last_poll_ms);
        let req = ureq::get(&endpoint);
        let req = if !self.api_key.is_empty() {
            req.set("Authorization", &format!("Bearer {}", self.api_key))
        } else {
            req
        };
        let resp = req.call().map_err(|e| format!("http: {e}"))?;
        let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
        let msgs: Vec<WaMessage> =
            serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;

        self.last_poll_ms = now_ms();
        Ok(msgs
            .into_iter()
            .map(|m| InboundMessage {
                metadata: Some(format!(
                    "(:channel-class \"whatsapp-bridge\" :node-id \"{}\" :remote t)",
                    m.from.replace('\\', "\\\\").replace('"', "\\\"")
                )),
                address: m.from,
                text: m.body,
            })
            .collect())
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let endpoint = format!("{}/api/sendText", self.api_url);
        let payload = serde_json::json!({ "to": channel, "text": text });
        let req = ureq::post(&endpoint).set("Content-Type", "application/json");
        let req = if !self.api_key.is_empty() {
            req.set("Authorization", &format!("Bearer {}", self.api_key))
        } else {
            req
        };
        req.send_string(&payload.to_string())
            .map_err(|e| format!("http: {e}"))?;
        Ok(())
    }

    async fn shutdown(&mut self) {
        self.api_url.clear();
        self.api_key.clear();
        self.last_poll_ms = 0;
    }
}
