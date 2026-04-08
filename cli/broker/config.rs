use harmonia_transport_auth::normalize_fingerprint;
use serde_json::json;

use super::{
    broker_trust_state, config_or, now_unix_ms, set_config,
    AgentConfigData, GraphqlResponse,
    BROKER_SCOPE, CONFIG_COMPONENT, DEFAULT_REMOTE_CONFIG_URL, DEFAULT_REMOTE_LABEL,
    FRONTEND_SCOPE, REMOTE_CONFIG_QUERY,
    sign_with_vault, resolve_wallet_db_path,
};

pub(super) fn sync_remote_config_once() -> Result<(), Box<dyn std::error::Error>> {
    let remote_config_url = config_or(BROKER_SCOPE, "remote-config-url", DEFAULT_REMOTE_CONFIG_URL);
    if remote_config_url.trim().is_empty() {
        refresh_trust_state_from_local_config();
        return Ok(());
    }
    let wallet = resolve_wallet_db_path();
    let identity_label = config_or(
        BROKER_SCOPE,
        "remote-config-identity-label",
        DEFAULT_REMOTE_LABEL,
    );

    let bootstrap = sign_with_vault(&wallet, &identity_label, "harmonia:init")?;
    let owner_fingerprint = normalize_fingerprint(&bootstrap.public_key);
    let requested_at = now_unix_ms().to_string();
    let message = format!(
        "harmonia:agent-config:get:{}:{}",
        owner_fingerprint, requested_at
    );
    let signed = sign_with_vault(&wallet, &identity_label, &message)?;

    let req_body = json!({
        "query": REMOTE_CONFIG_QUERY,
        "variables": {
            "fingerprint": owner_fingerprint,
            "publicKey": signed.public_key,
            "signature": signed.signature,
            "requestedAt": requested_at,
        }
    });

    let response: GraphqlResponse<AgentConfigData> = ureq::post(&remote_config_url)
        .set("Content-Type", "application/json")
        .send_json(req_body)
        .map_err(|e| format!("remote config request failed: {e}"))?
        .into_json()
        .map_err(|e| format!("remote config decode failed: {e}"))?;

    if let Some(errors) = response.errors {
        let message = errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("remote config API error: {message}").into());
    }

    let Some(data) = response.data else {
        refresh_trust_state_from_local_config();
        return Ok(());
    };
    let Some(config) = data.config else {
        refresh_trust_state_from_local_config();
        return Ok(());
    };

    let broker = format!("{}:{}", config.mqtt_domain, config.mqtt_port.max(1));
    let mut trusted_fps: Vec<String> = config
        .trusted_devices
        .iter()
        .map(|device| {
            device
                .mqtt_identity_fingerprint
                .clone()
                .filter(|fp| !fp.trim().is_empty())
                .unwrap_or_else(|| device.fingerprint.clone())
        })
        .collect();
    if trusted_fps.is_empty() {
        trusted_fps = config.trusted_client_fingerprints.clone();
    }
    trusted_fps = trusted_fps
        .into_iter()
        .map(|fp| normalize_fingerprint(&fp))
        .collect();
    trusted_fps.sort();
    trusted_fps.dedup();

    set_config(FRONTEND_SCOPE, "broker", &broker)?;
    set_config(
        FRONTEND_SCOPE,
        "tls",
        if config.mqtt_tls_required { "1" } else { "0" },
    )?;
    set_config(BROKER_SCOPE, "mode", &config.broker_mode)?;
    set_config(
        FRONTEND_SCOPE,
        "trusted-client-fingerprints-json",
        &serde_json::to_string(&trusted_fps)?,
    )?;
    set_config(
        FRONTEND_SCOPE,
        "trusted-device-registry-json",
        &serde_json::to_string(&config.trusted_devices)?,
    )?;
    if let Some(url) = config.push_webhook_url {
        set_config(FRONTEND_SCOPE, "push-webhook-url", &url)?;
        set_config(BROKER_SCOPE, "push-webhook-url", &url)?;
    }
    if let Some(token) = config.push_webhook_token {
        set_config(FRONTEND_SCOPE, "push-webhook-token", &token)?;
        set_config(BROKER_SCOPE, "push-webhook-token", &token)?;
    }
    if let Some(config_json) = config.config_json {
        set_config(BROKER_SCOPE, "remote-config-json", &config_json)?;
    }
    set_config(BROKER_SCOPE, "last-sync-fingerprint", &config.fingerprint)?;
    set_config(BROKER_SCOPE, "last-sync-at-ms", &now_unix_ms().to_string())?;
    set_config(BROKER_SCOPE, "last-sync-identity-label", &signed.label)?;
    set_config(
        BROKER_SCOPE,
        "last-sync-identity-index",
        &signed.index.to_string(),
    )?;

    let owner_fingerprint = normalize_fingerprint(&signed.public_key);
    update_trust_state(owner_fingerprint, trusted_fps);
    Ok(())
}

pub(super) fn refresh_trust_state_from_local_config() {
    let owner = config_or(BROKER_SCOPE, "identity-public-key", "");
    let trusted = harmonia_config_store::get_config(
        CONFIG_COMPONENT,
        FRONTEND_SCOPE,
        "trusted-client-fingerprints-json",
    )
    .ok()
    .flatten()
    .unwrap_or_else(|| "[]".to_string());
    let trusted = serde_json::from_str::<Vec<String>>(&trusted)
        .unwrap_or_default()
        .into_iter()
        .map(|fp| normalize_fingerprint(&fp))
        .collect::<Vec<_>>();
    update_trust_state(normalize_fingerprint(&owner), trusted);
}

pub(super) fn update_trust_state(owner_fingerprint: String, trusted_fingerprints: Vec<String>) {
    if let Ok(mut state) = broker_trust_state().write() {
        state.owner_fingerprint = owner_fingerprint;
        state.trusted_fingerprints = trusted_fingerprints.into_iter().collect();
    }
}
