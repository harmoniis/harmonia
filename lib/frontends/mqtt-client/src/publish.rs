use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::device::push_config;
use crate::model::{
    DeviceInfo, MessageEnvelope, VaultSignResult, COMPONENT, CONFIG_COMPONENT,
    DEFAULT_REMOTE_LABEL, PUSH_WEBHOOK_MUTATION,
};

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Send a push notification for an offline device.
pub(crate) fn send_offline_push(device: &DeviceInfo, payload: &str) {
    let _push_token = match &device.push_token {
        Some(t) if !t.is_empty() => t.clone(),
        _ => return,
    };
    let config = match push_config().read() {
        Ok(guard) => match guard.as_ref() {
            Some(c) => harmonia_push::PushConfig {
                webhook_url: c.webhook_url.clone(),
                auth_token: c.auth_token.clone(),
                timeout_ms: c.timeout_ms,
            },
            None => return,
        },
        Err(_) => return,
    };
    if config.webhook_url.trim().is_empty() {
        return;
    }
    let envelope: MessageEnvelope = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return,
    };
    let agent_fingerprint = envelope.agent_fp.trim().to_ascii_uppercase();
    if agent_fingerprint.is_empty() {
        return;
    }
    let client_fingerprint = if device.owner_fingerprint.trim().is_empty() {
        envelope.client_fp.trim().to_ascii_uppercase()
    } else {
        device.owner_fingerprint.trim().to_ascii_uppercase()
    };
    if client_fingerprint.is_empty() {
        return;
    }
    let body = truncate_for_push(&notification_body(&envelope));
    let message_json = match serde_json::to_string(&serde_json::json!({
        "title": "Harmonia",
        "body": body,
        "data": serde_json::Value::Null,
    })) {
        Ok(value) => value,
        Err(_) => return,
    };
    let message = format!(
        "harmonia:push:webhook:{}:{}:{}:{}",
        agent_fingerprint,
        client_fingerprint,
        device.device_id,
        sha256_hex(&message_json)
    );
    let identity_label = harmonia_config_store::get_config(
        CONFIG_COMPONENT,
        BROKER_SCOPE,
        "remote-config-identity-label",
    )
    .ok()
    .flatten()
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| DEFAULT_REMOTE_LABEL.to_string());
    let wallet_path = resolve_wallet_db_path();
    let signed = match sign_with_vault(&wallet_path, &identity_label, &message) {
        Ok(value) => value,
        Err(_) => return,
    };
    let request = serde_json::json!({
        "query": PUSH_WEBHOOK_MUTATION,
        "variables": {
            "agentFingerprint": agent_fingerprint,
            "clientFingerprint": client_fingerprint,
            "deviceId": device.device_id,
            "publicKey": signed.public_key,
            "signature": signed.signature,
            "title": "Harmonia",
            "body": body,
            "data": serde_json::Value::Null,
        }
    });
    let request_json = match serde_json::to_string(&request) {
        Ok(value) => value,
        Err(_) => return,
    };
    let mut req = ureq::post(&config.webhook_url)
        .timeout(std::time::Duration::from_millis(config.timeout_ms))
        .set("Content-Type", "application/json");
    if let Some(token) = &config.auth_token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }
    let _ = req.send_string(&request_json);
}

pub(crate) fn truncate_for_push(text: &str) -> String {
    if text.len() <= 256 {
        text.to_string()
    } else {
        format!("{}...", &text[..253])
    }
}

pub(crate) fn notification_body(envelope: &MessageEnvelope) -> String {
    envelope
        .body
        .get("text")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .unwrap_or_else(|| envelope.body.to_string())
}

pub(crate) fn sha256_hex(input: &str) -> String {
    hex::encode(Sha256::digest(input.as_bytes()))
}

pub(crate) fn sign_with_vault(
    wallet: &PathBuf,
    label: &str,
    message: &str,
) -> Result<VaultSignResult, String> {
    let output = Command::new(resolve_hrmw_bin())
        .args([
            "key",
            "vault-sign",
            "--wallet",
            wallet.to_string_lossy().as_ref(),
            "--label",
            label,
            "--message",
            message,
        ])
        .output()
        .map_err(|e| format!("failed to execute hrmw key vault-sign: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "hrmw key vault-sign failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(VaultSignResult {
        public_key: parse_hrmw_output_field(&stdout, "Vault public key:")?,
        signature: parse_hrmw_output_field(&stdout, "Signature:")?,
    })
}

pub(crate) fn resolve_hrmw_bin() -> String {
    harmonia_config_store::get_config_or("mqtt-frontend", "global", "hrmw-bin", "hrmw")
        .unwrap_or_else(|_| "hrmw".to_string())
}

pub(crate) fn parse_hrmw_output_field(output: &str, prefix: &str) -> Result<String, String> {
    output
        .lines()
        .find_map(|line| line.strip_prefix(prefix))
        .map(|value| value.trim().to_string())
        .ok_or_else(|| format!("missing hrmw output field: {prefix}"))
}

pub(crate) fn resolve_wallet_db_path() -> PathBuf {
    if let Ok(path) = harmonia_config_store::get_config(COMPONENT, "global", "wallet-root") {
        if let Some(root) = path
            .map(|value| value.trim().to_string())
            .filter(|v| !v.is_empty())
        {
            return PathBuf::from(root).join("master.db");
        }
    }
    if let Ok(path) = std::env::var("HARMONIA_WALLET_ROOT") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join("master.db");
        }
    }
    for key in [
        "HARMONIA_VAULT_WALLET_DB",
        "HARMONIA_WALLET_DB",
        "HARMONIIS_WALLET_DB",
    ] {
        if let Ok(path) = std::env::var(key) {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                let candidate = PathBuf::from(trimmed);
                if candidate
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.eq_ignore_ascii_case("master.db"))
                    .unwrap_or(false)
                {
                    return candidate;
                }
                return candidate.join("master.db");
            }
        }
    }
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".harmoniis").join("wallet").join("master.db")
}

// ─── Config sexp parser ─────────────────────────────────────────────

pub(crate) fn extract_sexp_value(sexp: &str, key: &str) -> Option<String> {
    let pat = format!(":{}", key);
    let idx = sexp.find(&pat)?;
    let after = &sexp[idx + pat.len()..];
    let after = after.trim_start();
    if after.starts_with('"') {
        let inner = &after[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else if after.starts_with('(') {
        let close = after.find(')')?;
        let inner = &after[1..close];
        Some(
            inner
                .split('"')
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(","),
        )
    } else {
        let end = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

// Re-export BROKER_SCOPE for use by send_offline_push (used via model import above)
use crate::model::BROKER_SCOPE;
