use crate::rpc::{get_json, post_json, RequestFailure};
use crate::state::resolve_signal_config;

/// Request a device linking URI from signal-cli bridge.
/// The URI (sgnl://linkdevice?uuid=...&pub_key=...) is rendered as a QR code
/// and scanned from the Signal mobile app to link this device.
pub fn pair_init() -> Result<Option<String>, String> {
    let (rpc_url, account, auth_token) = resolve_signal_config();

    if rpc_url.is_empty() {
        return Err("signal rpc-url not configured".into());
    }

    // signal-cli REST API: POST /v1/qrcodelink or GET /v1/qrcodelink
    // Also try /v1/devices/link which some versions use
    let link_endpoints = [
        (true, format!("{rpc_url}/v1/qrcodelink")),
        (true, format!("{rpc_url}/v2/qrcodelink")),
        (false, format!("{rpc_url}/v1/devices/link")),
    ];

    for (is_post, endpoint) in &link_endpoints {
        let result = if *is_post {
            let body = serde_json::json!({ "deviceName": "harmonia" });
            match post_json(endpoint, &auth_token, &body) {
                Ok(()) => {
                    // POST succeeded but we need the URI from the response
                    // Try GET to retrieve it
                    get_json(endpoint, &auth_token).ok()
                }
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
            // Extract the device link URI
            let uri = json
                .get("uri")
                .or_else(|| json.get("qrCodeLink"))
                .or_else(|| json.get("deviceLink"))
                .or_else(|| json.get("data"))
                .and_then(|v| v.as_str());
            if let Some(uri) = uri {
                return Ok(Some(uri.to_string()));
            }
            // Some versions return the URI as a plain string
            if let Some(s) = json.as_str() {
                if s.starts_with("sgnl://") || s.starts_with("https://signal.") {
                    return Ok(Some(s.to_string()));
                }
            }
        }
    }

    // Fallback: try signal-cli command directly if available
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

/// Check if the Signal account is registered/linked.
pub fn pair_status() -> Result<(bool, String), String> {
    let (rpc_url, _account, auth_token) = resolve_signal_config();

    if rpc_url.is_empty() {
        return Ok((false, "rpc-url not configured".into()));
    }

    // Check account registration status
    let status_endpoints = [
        format!("{rpc_url}/v1/accounts/{_account}"),
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
