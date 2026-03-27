use serde::Deserialize;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const COMPONENT: &str = "whatsapp-frontend";
const WHATSAPP_API_KEY_SYMBOLS: &[&str] = &["whatsapp-session", "whatsapp-api-key"];
const WHATSAPP_API_URL_SYMBOLS: &[&str] = &["whatsapp-bridge-url", "whatsapp-api-url"];

/// Incoming message from the WhatsApp bridge API.
#[derive(Debug, Deserialize)]
struct WaMessage {
    from: String,
    body: String,
}

/// Runtime state for the WhatsApp frontend.
pub struct WhatsAppState {
    pub api_url: String,
    pub api_key: String,
    pub last_poll_ms: u64,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<WhatsAppState>> = OnceLock::new();

fn state() -> &'static RwLock<WhatsAppState> {
    STATE.get_or_init(|| {
        RwLock::new(WhatsAppState {
            api_url: String::new(),
            api_key: String::new(),
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

/// Initialise the WhatsApp client from an s-expression config string.
///
/// Recognised keys: `:api-url`, `:api-key`.
pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;

    if let Some(api_url) = sexp_value(config, ":api-url") {
        let trimmed = api_url.trim();
        if !trimmed.is_empty() {
            let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "api-url", trimmed);
        }
    }
    if let Some(api_key) = sexp_value(config, ":api-key") {
        let trimmed = api_key.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("whatsapp-session", trimmed)?;
        }
    }

    s.api_url = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .or_else(|| {
            read_vault_secret(WHATSAPP_API_URL_SYMBOLS)
                .ok()
                .flatten()
                .and_then(|legacy| {
                    let _ =
                        harmonia_config_store::set_config(COMPONENT, COMPONENT, "api-url", &legacy);
                    Some(legacy)
                })
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".into());

    // Strip trailing slash for consistency.
    if s.api_url.ends_with('/') {
        s.api_url.pop();
    }

    s.api_key = read_vault_secret(WHATSAPP_API_KEY_SYMBOLS)?.unwrap_or_default();

    s.last_poll_ms = now_ms();
    s.initialized = true;
    Ok(())
}

/// Poll the WhatsApp bridge for new messages since last poll.
///
/// Returns a vec of `(phone, text, metadata)` triples.
pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (url, key, since) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("not initialised".into());
        }
        (s.api_url.clone(), s.api_key.clone(), s.last_poll_ms)
    };

    let endpoint = format!("{}/api/messages?since={}", url, since);
    let req = ureq::get(&endpoint);
    let req = if !key.is_empty() {
        req.set("Authorization", &format!("Bearer {}", key))
    } else {
        req
    };

    let resp = req.call().map_err(|e| format!("http: {e}"))?;
    let body = resp.into_string().map_err(|e| format!("body: {e}"))?;
    let msgs: Vec<WaMessage> = serde_json::from_str(&body).map_err(|e| format!("json: {e}"))?;

    // Update last_poll_ms.
    if let Ok(mut s) = state().write() {
        s.last_poll_ms = now_ms();
    }

    Ok(msgs
        .into_iter()
        .map(|m| {
            let metadata = format!(
                "(:channel-class \"whatsapp-bridge\" :node-id \"{}\" :remote t)",
                m.from.replace('\\', "\\\\").replace('"', "\\\"")
            );
            (m.from, m.body, Some(metadata))
        })
        .collect())
}

/// Send a text message via the WhatsApp bridge.
pub fn send(phone: &str, text: &str) -> Result<(), String> {
    let (url, key) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("not initialised".into());
        }
        (s.api_url.clone(), s.api_key.clone())
    };

    let endpoint = format!("{}/api/sendText", url);
    let payload = serde_json::json!({
        "to": phone,
        "text": text,
    });

    let req = ureq::post(&endpoint).set("Content-Type", "application/json");
    let req = if !key.is_empty() {
        req.set("Authorization", &format!("Bearer {}", key))
    } else {
        req
    };

    req.send_string(&payload.to_string())
        .map_err(|e| format!("http: {e}"))?;
    Ok(())
}

/// Shutdown: reset state.
pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.api_url.clear();
        s.api_key.clear();
        s.last_poll_ms = 0;
        s.initialized = false;
    }
}

/// Returns true when `init` has been called successfully.
pub fn is_initialized() -> bool {
    state().read().map(|s| s.initialized).unwrap_or(false)
}

/// Request a QR code for WhatsApp device pairing from the bridge.
/// Most WhatsApp bridges (whatsmeow, baileys) expose a /api/pair or /api/qr endpoint.
/// Resolve API URL and key from in-process state, falling back to config-store.
/// This allows pair_init/pair_status to work from the CLI process where the
/// WhatsApp frontend .so was never init()'d by the gateway.
fn resolve_api_config() -> (String, String) {
    // Try in-process state first (populated when loaded as gateway plugin)
    if let Ok(s) = state().read() {
        if !s.api_url.is_empty() {
            return (s.api_url.clone(), s.api_key.clone());
        }
    }
    // Fall back to config-store (works from CLI/TUI process)
    let url = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_default();
    let key = harmonia_config_store::get_own(COMPONENT, "api-key")
        .ok()
        .flatten()
        .or_else(|| {
            WHATSAPP_API_KEY_SYMBOLS.iter().find_map(|sym| {
                harmonia_vault::get_secret_for_component(COMPONENT, sym)
                    .ok()
                    .flatten()
            })
        })
        .unwrap_or_default();
    (url, key)
}

pub fn pair_init() -> Result<Option<String>, String> {
    let (url, key) = resolve_api_config();

    if url.is_empty() {
        return Err("whatsapp api-url not configured".into());
    }

    // Try common bridge QR/pairing endpoints
    let endpoints = [
        format!("{url}/api/pair"),
        format!("{url}/api/qr"),
        format!("{url}/api/login/qr"),
        format!("{url}/api/v1/qr"),
    ];

    for endpoint in &endpoints {
        let req = ureq::get(endpoint);
        let req = if !key.is_empty() {
            req.set("Authorization", &format!("Bearer {key}"))
        } else {
            req
        };
        match req.call() {
            Ok(resp) => {
                let body = resp.into_string().unwrap_or_default();
                if body.trim().is_empty() {
                    continue;
                }
                // Try to extract QR data from JSON response
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(qr) = json
                        .get("qr")
                        .or_else(|| json.get("qrCode"))
                        .or_else(|| json.get("qr_code"))
                        .or_else(|| json.get("data"))
                        .and_then(|v| v.as_str())
                    {
                        return Ok(Some(qr.to_string()));
                    }
                }
                // Raw text response (some bridges return just the QR data string)
                let trimmed = body.trim();
                if !trimmed.is_empty() && !trimmed.starts_with('<') {
                    return Ok(Some(trimmed.to_string()));
                }
            }
            Err(ureq::Error::Status(404, _)) => continue,
            Err(e) => return Err(format!("whatsapp pair request failed: {e}")),
        }
    }

    Err("whatsapp bridge does not expose a pairing endpoint (tried /api/pair, /api/qr, /api/login/qr, /api/v1/qr)".into())
}

/// Check if the WhatsApp bridge session is connected (paired).
pub fn pair_status() -> Result<(bool, String), String> {
    let (url, key) = resolve_api_config();

    if url.is_empty() {
        return Ok((false, "api-url not configured".into()));
    }

    let endpoints = [
        format!("{url}/api/status"),
        format!("{url}/api/v1/status"),
        format!("{url}/api/health"),
    ];

    for endpoint in &endpoints {
        let req = ureq::get(endpoint);
        let req = if !key.is_empty() {
            req.set("Authorization", &format!("Bearer {key}"))
        } else {
            req
        };
        match req.call() {
            Ok(resp) => {
                let body = resp.into_string().unwrap_or_default();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                    let connected = json
                        .get("connected")
                        .or_else(|| json.get("loggedIn"))
                        .or_else(|| json.get("paired"))
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let msg = json
                        .get("message")
                        .or_else(|| json.get("status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(if connected {
                            "connected"
                        } else {
                            "not connected"
                        });
                    return Ok((connected, msg.to_string()));
                }
            }
            Err(ureq::Error::Status(404, _)) => continue,
            Err(_) => continue,
        }
    }

    Ok((false, "could not determine status".into()))
}
