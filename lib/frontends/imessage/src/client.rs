use serde::Deserialize;
use std::env;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ImessageState {
    pub server_url: String,
    pub password: String,
    pub last_poll_ms: u64,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<ImessageState>> = OnceLock::new();

fn state() -> &'static RwLock<ImessageState> {
    STATE.get_or_init(|| {
        RwLock::new(ImessageState {
            server_url: String::new(),
            password: String::new(),
            last_poll_ms: 0,
            initialized: false,
        })
    })
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Extract a value for a key from a simple s-expression config string.
/// E.g. from `(:server-url "http://localhost:1234" :password "secret")`
/// extract_sexp_value(config, ":server-url") -> Some("http://localhost:1234")
fn extract_sexp_value(config: &str, key: &str) -> Option<String> {
    let idx = config.find(key)?;
    let after_key = &config[idx + key.len()..];
    // skip whitespace, find opening quote
    let after_key = after_key.trim_start();
    if !after_key.starts_with('"') {
        return None;
    }
    let after_quote = &after_key[1..];
    let end = after_quote.find('"')?;
    Some(after_quote[..end].to_string())
}

/// Initialize the iMessage client from a config s-expression.
pub fn init(config: &str) -> Result<(), String> {
    let st = state();
    let mut guard = st.write().map_err(|e| format!("lock poisoned: {e}"))?;

    if guard.initialized {
        return Err("imessage already initialized".into());
    }

    let server_url = extract_sexp_value(config, ":server-url")
        .or_else(|| env::var("HARMONIA_IMESSAGE_SERVER_URL").ok())
        .unwrap_or_default();

    let password = extract_sexp_value(config, ":password")
        .or_else(|| env::var("HARMONIA_IMESSAGE_PASSWORD").ok())
        .unwrap_or_default();

    if server_url.is_empty() {
        return Err(
            "imessage: server-url is required (config or HARMONIA_IMESSAGE_SERVER_URL)".into(),
        );
    }

    guard.server_url = server_url;
    guard.password = password;
    guard.last_poll_ms = now_ms();
    guard.initialized = true;
    Ok(())
}

// BlueBubbles message response structures
#[derive(Deserialize)]
struct BbResponse {
    #[serde(default)]
    data: Vec<BbMessage>,
}

#[derive(Deserialize)]
struct BbMessage {
    #[serde(default)]
    handle: Option<BbHandle>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default, rename = "dateCreated")]
    date_created: Option<u64>,
    #[serde(default, rename = "isFromMe")]
    is_from_me: Option<bool>,
}

#[derive(Deserialize)]
struct BbHandle {
    #[serde(default)]
    address: Option<String>,
}

/// Poll BlueBubbles for new messages since last_poll_ms.
/// Returns (phone_or_email, text) pairs for inbound messages.
pub fn poll() -> Result<Vec<(String, String)>, String> {
    let st = state();
    let (url, password, after) = {
        let guard = st.read().map_err(|e| format!("lock poisoned: {e}"))?;
        if !guard.initialized {
            return Err("imessage: not initialized".into());
        }
        (
            guard.server_url.clone(),
            guard.password.clone(),
            guard.last_poll_ms,
        )
    };

    let endpoint = format!(
        "{}/api/v1/message?after={}&password={}&sort=asc&limit=100",
        url.trim_end_matches('/'),
        after,
        password
    );

    let resp = ureq::get(&endpoint)
        .call()
        .map_err(|e| format!("imessage poll HTTP error: {e}"))?;

    let body: BbResponse = resp
        .into_json()
        .map_err(|e| format!("imessage poll JSON parse error: {e}"))?;

    let mut results = Vec::new();
    let mut max_ts = after;

    for msg in &body.data {
        // Skip messages from us
        if msg.is_from_me == Some(true) {
            continue;
        }
        let address = msg
            .handle
            .as_ref()
            .and_then(|h| h.address.clone())
            .unwrap_or_else(|| "unknown".into());
        let text = msg.text.clone().unwrap_or_default();
        if !text.is_empty() {
            results.push((address, text));
        }
        if let Some(ts) = msg.date_created {
            if ts > max_ts {
                max_ts = ts;
            }
        }
    }

    // Update last_poll_ms
    if max_ts > after {
        if let Ok(mut guard) = st.write() {
            guard.last_poll_ms = max_ts + 1; // +1 to avoid re-fetching the same message
        }
    }

    Ok(results)
}

/// Send a message via BlueBubbles.
/// `channel` is a phone number or email address.
pub fn send(channel: &str, payload: &str) -> Result<(), String> {
    let st = state();
    let (url, password) = {
        let guard = st.read().map_err(|e| format!("lock poisoned: {e}"))?;
        if !guard.initialized {
            return Err("imessage: not initialized".into());
        }
        (guard.server_url.clone(), guard.password.clone())
    };

    let endpoint = format!("{}/api/v1/message/text", url.trim_end_matches('/'));

    let body = serde_json::json!({
        "chatGuid": format!("iMessage;-;{channel}"),
        "message": payload,
        "password": password,
    });

    let resp = ureq::post(&endpoint)
        .send_json(&body)
        .map_err(|e| format!("imessage send HTTP error: {e}"))?;

    if resp.status() >= 400 {
        return Err(format!("imessage send failed: HTTP {}", resp.status()));
    }

    Ok(())
}

/// Shut down: reset state so it can be re-initialized.
pub fn shutdown() {
    if let Ok(mut guard) = state().write() {
        guard.server_url.clear();
        guard.password.clear();
        guard.last_poll_ms = 0;
        guard.initialized = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sexp_value() {
        let config = r#"(:server-url "http://localhost:1234" :password "secret")"#;
        assert_eq!(
            extract_sexp_value(config, ":server-url"),
            Some("http://localhost:1234".into())
        );
        assert_eq!(
            extract_sexp_value(config, ":password"),
            Some("secret".into())
        );
        assert_eq!(extract_sexp_value(config, ":missing"), None);
    }
}
