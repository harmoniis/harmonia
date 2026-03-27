use serde::Deserialize;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "telegram-frontend";
const TELEGRAM_BOT_TOKEN_SYMBOLS: &[&str] = &["telegram-bot-token", "telegram-bot-api-token"];

// ---------------------------------------------------------------------------
// Telegram Bot API response types
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

pub struct TelegramState {
    pub bot_token: String,
    pub last_update_id: i64,
    pub initialized: bool,
}

/// Legacy singleton — deprecated. Frontend actor should own this state.
static LEGACY_STATE: OnceLock<RwLock<TelegramState>> = OnceLock::new();

fn state() -> &'static RwLock<TelegramState> {
    LEGACY_STATE.get_or_init(|| {
        RwLock::new(TelegramState {
            bot_token: String::new(),
            last_update_id: 0,
            initialized: false,
        })
    })
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

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Initialise the Telegram bot from an s-expression config string.
///
/// Recognised key: `:bot-token` (ingested into vault as `telegram-bot-token`).
pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;

    if let Some(token) = sexp_value(config, ":bot-token") {
        if !token.trim().is_empty() {
            harmonia_vault::set_secret_for_symbol("telegram-bot-token", token.trim())?;
        }
    }

    s.bot_token = read_vault_secret(TELEGRAM_BOT_TOKEN_SYMBOLS)?.unwrap_or_default();

    if s.bot_token.is_empty() {
        return Err("no bot token provided in vault (symbol: telegram-bot-token)".into());
    }

    s.last_update_id = 0;
    s.initialized = true;
    Ok(())
}

/// Poll Telegram for new updates via `getUpdates`.
///
/// Uses a 1-second long-poll timeout so the call is non-blocking in practice.
/// Returns a vec of `(chat_id_string, text, metadata)` triples.
pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (token, offset) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("not initialised".into());
        }
        (s.bot_token.clone(), s.last_update_id)
    };

    let url = format!(
        "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=1",
        token,
        offset + 1
    );

    let resp = ureq::get(&url).call().map_err(|e| format!("http: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
    let tg: TgResponse = serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;

    if !tg.ok {
        return Err(format!(
            "telegram api error: {}",
            tg.description.unwrap_or_else(|| "unknown".into())
        ));
    }

    let updates = tg.result.unwrap_or_default();
    let mut results = Vec::new();
    let mut max_id = offset;

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
                results.push((chat_id, text.clone(), Some(metadata)));
            }
        }
    }

    // Persist the highest update_id we have seen.
    if max_id > offset {
        if let Ok(mut s) = state().write() {
            s.last_update_id = max_id;
        }
    }

    Ok(results)
}

/// Send a text message to a Telegram chat.
pub fn send(chat_id: &str, text: &str) -> Result<(), String> {
    let token = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("not initialised".into());
        }
        s.bot_token.clone()
    };

    let url = format!("https://api.telegram.org/bot{}/sendMessage", token);
    let payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
    });

    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|e| format!("http: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
    let tg: TgSendResponse = serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;

    if !tg.ok {
        return Err(format!(
            "sendMessage failed: {}",
            tg.description.unwrap_or_else(|| "unknown".into())
        ));
    }
    Ok(())
}

/// Shutdown: reset state.
pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.bot_token.clear();
        s.last_update_id = 0;
        s.initialized = false;
    }
}

/// Returns true when `init` has been called successfully.
pub fn is_initialized() -> bool {
    state().read().map(|s| s.initialized).unwrap_or(false)
}
