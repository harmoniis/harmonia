use harmonia_transport_auth::{load_trusted_fingerprints, normalize_fingerprint};
use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};

use crate::model::{DeviceInfo, MessageEnvelope, RemoteDeviceRecord, COMPONENT};
use crate::queue::flush_offline_queue;

pub(crate) fn device_registry() -> &'static RwLock<HashMap<String, DeviceInfo>> {
    static REG: OnceLock<RwLock<HashMap<String, DeviceInfo>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn push_config() -> &'static RwLock<Option<harmonia_push::PushConfig>> {
    static CFG: OnceLock<RwLock<Option<harmonia_push::PushConfig>>> = OnceLock::new();
    CFG.get_or_init(|| RwLock::new(None))
}

pub(crate) fn load_remote_device_registry() {
    let raw = match harmonia_config_store::get_own(COMPONENT, "trusted-device-registry-json") {
        Ok(Some(raw)) if !raw.trim().is_empty() => raw,
        _ => return,
    };
    let Ok(devices) = serde_json::from_str::<Vec<RemoteDeviceRecord>>(&raw) else {
        return;
    };
    if let Ok(mut reg) = device_registry().write() {
        for remote in devices {
            reg.entry(remote.device_id.clone()).or_insert(DeviceInfo {
                device_id: remote.device_id.clone(),
                owner_fingerprint: remote.fingerprint.clone(),
                platform: remote.platform.unwrap_or_else(|| "unknown".to_string()),
                platform_version: String::new(),
                app_version: remote
                    .label
                    .clone()
                    .unwrap_or_else(|| remote.fingerprint.clone()),
                device_model: String::new(),
                capabilities: vec![],
                permissions_granted: vec![],
                a2ui_version: String::new(),
                push_token: remote.push_token,
                mqtt_identity_fingerprint: remote.mqtt_identity_fingerprint,
                connected: false,
                last_seen_ms: 0,
            });
        }
    }
}

pub(crate) fn trusted_origin_fingerprints() -> HashSet<String> {
    load_trusted_fingerprints(COMPONENT, harmonia_transport_auth::DEFAULT_TRUST_SCOPE_KEY)
}

/// Extract device_id from an MQTT topic path.
/// Pattern: `harmonia/{agent_id}/device/{device_id}/...`
pub(crate) fn extract_device_id_from_topic(topic: &str) -> Option<String> {
    let parts: Vec<&str> = topic.split('/').collect();
    // Look for "device" segment, take the next one as device_id
    for (i, part) in parts.iter().enumerate() {
        if *part == "device" && i + 1 < parts.len() {
            return Some(parts[i + 1].to_string());
        }
    }
    None
}

/// Check if a topic is a device connect event.
pub(crate) fn is_device_connect_topic(topic: &str) -> bool {
    topic.ends_with("/connect") && topic.contains("/device/")
}

/// Check if a topic is a device disconnect event.
pub(crate) fn is_device_disconnect_topic(topic: &str) -> bool {
    topic.ends_with("/disconnect") && topic.contains("/device/")
}

/// Wave 4.1: Validate agent_fp against vault-stored expected fingerprint.
/// Returns true if fingerprint is valid or no expected fingerprint is configured.
pub(crate) fn validate_agent_fingerprint(envelope: &MessageEnvelope) -> bool {
    let _ = harmonia_vault::init_from_env();
    let expected_fp =
        match harmonia_vault::get_secret_for_component("mqtt-frontend", "mqtt_agent_fp")
            .ok()
            .flatten()
        {
            Some(fp) if !fp.is_empty() => normalize_fingerprint(&fp),
            _ => return true, // No expected fingerprint configured, allow
        };
    if envelope.agent_fp.is_empty() {
        log::warn!("mqtt: message missing agent_fp, downgrading to untrusted");
        return false;
    }
    let actual = normalize_fingerprint(&envelope.agent_fp);
    if actual != expected_fp {
        log::warn!(
            "mqtt: agent_fp mismatch (got {}, expected {}), downgrading to untrusted",
            actual,
            expected_fp
        );
        return false;
    }
    true
}

pub(crate) fn origin_is_trusted(env: &MessageEnvelope) -> bool {
    let trusted = trusted_origin_fingerprints();
    if trusted.is_empty() {
        return true;
    }
    trusted.contains(&normalize_fingerprint(&env.client_fp))
}

pub(crate) fn build_envelope_metadata(
    env: &MessageEnvelope,
    fp_valid: bool,
    origin_trusted: bool,
) -> String {
    format!(
        "(:origin-fp \"{}\" :agent-fp \"{}\" :fingerprint-valid {} :trusted-origin {})",
        env.client_fp,
        env.agent_fp,
        if fp_valid { "t" } else { "nil" },
        if origin_trusted { "t" } else { "nil" }
    )
}

pub(crate) fn merge_metadata_sexp(a: Option<&str>, b: Option<&str>) -> Option<String> {
    fn trim_parens(s: &str) -> &str {
        let t = s.trim();
        if t.starts_with('(') && t.ends_with(')') && t.len() >= 2 {
            &t[1..t.len() - 1]
        } else {
            t
        }
    }
    match (a, b) {
        (None, None) => None,
        (Some(x), None) => Some(x.to_string()),
        (None, Some(y)) => Some(y.to_string()),
        (Some(x), Some(y)) => Some(format!("({} {})", trim_parens(x), trim_parens(y))),
    }
}

/// Handle device connect: parse JSON payload, register device.
pub(crate) fn handle_device_connect(payload: &str) {
    let mut device: DeviceInfo = match serde_json::from_str(payload) {
        Ok(d) => d,
        Err(_) => return,
    };
    device.connected = true;
    device.last_seen_ms = crate::publish::now_ms();
    let device_id = device.device_id.clone();

    if let Ok(mut reg) = device_registry().write() {
        if let Some(existing) = reg.get(&device_id) {
            if device.owner_fingerprint.trim().is_empty() {
                device.owner_fingerprint = existing.owner_fingerprint.clone();
            }
            if device.push_token.is_none() {
                device.push_token = existing.push_token.clone();
            }
            if device.mqtt_identity_fingerprint.is_none() {
                device.mqtt_identity_fingerprint = existing.mqtt_identity_fingerprint.clone();
            }
        }
        reg.insert(device_id.clone(), device);
    }

    // Flush any offline-queued messages for this device
    flush_offline_queue(&device_id);
}

/// Handle device disconnect: mark as not connected.
pub(crate) fn handle_device_disconnect(topic: &str) {
    if let Some(device_id) = extract_device_id_from_topic(topic) {
        if let Ok(mut reg) = device_registry().write() {
            if let Some(device) = reg.get_mut(&device_id) {
                device.connected = false;
                device.last_seen_ms = crate::publish::now_ms();
            }
        }
    }
}
