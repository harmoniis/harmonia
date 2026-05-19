//! Stateless pairing helpers — `pair_init` returns a QR string from the
//! WhatsApp bridge, `pair_status` reports whether the bridge has a paired
//! session. Both are called from the CLI / TUI side and read all config
//! from config-store / vault, so they don't need the actor to be alive.

use crate::frontend::{read_vault_secret, COMPONENT, WHATSAPP_API_KEY_SYMBOLS};

fn resolve_api_config() -> (String, String) {
    let url = harmonia_config_store::get_own(COMPONENT, "api-url")
        .ok()
        .flatten()
        .unwrap_or_default();
    let key = harmonia_config_store::get_own(COMPONENT, "api-key")
        .ok()
        .flatten()
        .or_else(|| {
            read_vault_secret(WHATSAPP_API_KEY_SYMBOLS)
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    (url, key)
}

pub fn pair_init() -> Result<Option<String>, String> {
    let (url, key) = resolve_api_config();
    if url.is_empty() {
        return Err("whatsapp api-url not configured".into());
    }
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
