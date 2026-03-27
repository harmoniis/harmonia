use serde::Deserialize;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const COMPONENT: &str = "imessage-frontend";
const IMESSAGE_PASSWORD_SYMBOLS: &[&str] = &["bluebubbles-password", "imessage-password"];
const IMESSAGE_SERVER_URL_SYMBOLS: &[&str] = &["bluebubbles-server-url", "imessage-server-url"];

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

fn extract_sexp_value(config: &str, key: &str) -> Option<String> {
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

/// Initialize the iMessage client from a config s-expression.
/// iMessage (via BlueBubbles) only works on macOS. On other platforms, iMessage
/// signals arrive via the Tailscale mesh from a macOS node as remote signals.
pub fn init(config: &str) -> Result<(), String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = config;
        return Err("imessage frontend requires macOS (BlueBubbles). On Linux, iMessage signals arrive via Tailscale mesh from a macOS node.".into());
    }

    #[cfg(target_os = "macos")]
    {
        init_macos(config)
    }
}

#[cfg(target_os = "macos")]
fn init_macos(config: &str) -> Result<(), String> {
    let st = state();
    let mut guard = st.write().map_err(|e| format!("lock poisoned: {e}"))?;

    if guard.initialized {
        return Err("imessage already initialized".into());
    }

    if let Some(url) = extract_sexp_value(config, ":server-url") {
        let trimmed = url.trim();
        if !trimmed.is_empty() {
            let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "server-url", trimmed);
        }
    }
    if let Some(password) = extract_sexp_value(config, ":password") {
        let trimmed = password.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("bluebubbles-password", trimmed)?;
        }
    }

    let server_url = harmonia_config_store::get_own(COMPONENT, "server-url")
        .ok()
        .flatten()
        .or_else(|| {
            read_vault_secret(IMESSAGE_SERVER_URL_SYMBOLS)
                .ok()
                .flatten()
                .and_then(|legacy| {
                    let _ = harmonia_config_store::set_config(
                        COMPONENT,
                        COMPONENT,
                        "server-url",
                        &legacy,
                    );
                    Some(legacy)
                })
        })
        .unwrap_or_default();

    let password = read_vault_secret(IMESSAGE_PASSWORD_SYMBOLS)?.unwrap_or_default();

    if server_url.is_empty() {
        return Err(
            "imessage: server-url is required (config-store imessage-frontend/server-url)".into(),
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
/// Returns (phone_or_email, text, metadata) triples for inbound messages.
pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
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
            let metadata = format!(
                "(:channel-class \"imessage-bridge\" :node-id \"{}\" :remote t)",
                address.replace('\\', "\\\\").replace('"', "\\\"")
            );
            results.push((address, text, Some(metadata)));
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
