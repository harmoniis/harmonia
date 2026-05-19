//! Stateless Signal pairing helpers — `pair_init` returns the device-link URI
//! (sgnl://… QR payload) from the signal-cli REST proxy, `pair_status`
//! reports whether the account is linked. Both read all config from
//! config-store / vault directly so the CLI can call them without the
//! frontend actor being alive.

use crate::frontend::{
    read_vault_secret, COMPONENT, SIGNAL_AUTH_TOKEN_SYMBOLS,
};
use crate::rpc::{get_json, post_json, RequestFailure};

fn resolve_signal_config() -> (String, String, String) {
    let rpc_url = harmonia_config_store::get_own(COMPONENT, "rpc-url")
        .ok()
        .flatten()
        .unwrap_or_default();
    let account = harmonia_config_store::get_own(COMPONENT, "account")
        .ok()
        .flatten()
        .unwrap_or_default();
    let auth_token = harmonia_config_store::get_own(COMPONENT, "auth-token")
        .ok()
        .flatten()
        .or_else(|| {
            read_vault_secret(SIGNAL_AUTH_TOKEN_SYMBOLS)
                .ok()
                .flatten()
        })
        .unwrap_or_default();
    (rpc_url, account, auth_token)
}

pub fn pair_init() -> Result<Option<String>, String> {
    let (rpc_url, account, auth_token) = resolve_signal_config();
    if rpc_url.is_empty() {
        return Err("signal rpc-url not configured".into());
    }
    let link_endpoints = [
        (true, format!("{rpc_url}/v1/qrcodelink")),
        (true, format!("{rpc_url}/v2/qrcodelink")),
        (false, format!("{rpc_url}/v1/devices/link")),
    ];
    for (is_post, endpoint) in &link_endpoints {
        let result = if *is_post {
            let body = serde_json::json!({ "deviceName": "harmonia" });
            match post_json(endpoint, &auth_token, &body) {
                Ok(()) => get_json(endpoint, &auth_token).ok(),
                Err(RequestFailure::NotFound) => continue,
                Err(RequestFailure::Other(e)) => return Err(e),
            }
        } else {
            match get_json(endpoint, &auth_token) {
                Ok(v) => Some(v),
                Err(RequestFailure::NotFound) => continue,
                Err(RequestFailure::Other(e)) => return Err(e),
            }
        };
        if let Some(json) = result {
            let uri = json
                .get("uri")
                .or_else(|| json.get("qrCodeLink"))
                .or_else(|| json.get("deviceLink"))
                .or_else(|| json.get("data"))
                .and_then(|v| v.as_str());
            if let Some(uri) = uri {
                return Ok(Some(uri.to_string()));
            }
            if let Some(s) = json.as_str() {
                if s.starts_with("sgnl://") || s.starts_with("https://signal.") {
                    return Ok(Some(s.to_string()));
                }
            }
        }
    }
    if !account.is_empty() {
        if let Ok(output) = std::process::Command::new("signal-cli")
            .args(["link", "-n", "harmonia"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("sgnl://") || trimmed.starts_with("https://signal.") {
                    return Ok(Some(trimmed.to_string()));
                }
            }
        }
    }
    Err("could not obtain signal device link URI (tried bridge REST API and signal-cli)".into())
}

pub fn pair_status() -> Result<(bool, String), String> {
    let (rpc_url, account, auth_token) = resolve_signal_config();
    if rpc_url.is_empty() {
        return Ok((false, "rpc-url not configured".into()));
    }
    let status_endpoints = [
        format!("{rpc_url}/v1/accounts/{account}"),
        format!("{rpc_url}/v1/about"),
    ];
    for endpoint in &status_endpoints {
        match get_json(endpoint, &auth_token) {
            Ok(json) => {
                let registered = json
                    .get("registered")
                    .or_else(|| json.get("linked"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let msg = if registered {
                    "device linked"
                } else {
                    "not linked"
                };
                return Ok((registered, msg.to_string()));
            }
            Err(RequestFailure::NotFound) => continue,
            Err(_) => continue,
        }
    }
    Ok((false, "could not determine link status".into()))
}
