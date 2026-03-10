use serde::Serialize;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;

const COMPONENT: &str = "push-frontend";

/// Push delivery configuration.
pub struct PushConfig {
    pub webhook_url: String,
    pub auth_token: Option<String>,
    pub timeout_ms: u64,
}

/// A single push notification payload.
#[derive(Debug, Clone, Serialize)]
pub struct PushPayload {
    pub device_token: String,
    pub platform: String,
    pub title: String,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

/// Send a push notification via HTTP webhook POST.
///
/// If `HARMONIA_PUSH_MODE=log`, writes the payload to a log file instead
/// of making an HTTP request. Useful for testing and development.
pub fn send_push(config: &PushConfig, payload: &PushPayload) -> Result<(), String> {
    // Test mode: write to file instead of HTTP
    if harmonia_config_store::get_own(COMPONENT, "mode")
        .ok()
        .flatten()
        .map(|v| v.eq_ignore_ascii_case("log"))
        .unwrap_or(false)
    {
        return log_push(payload);
    }

    if config.webhook_url.is_empty() {
        return Err("push webhook URL not configured".to_string());
    }

    let json = serde_json::to_string(payload)
        .map_err(|e| format!("push payload serialize failed: {e}"))?;

    let mut req = ureq::post(&config.webhook_url)
        .timeout(std::time::Duration::from_millis(config.timeout_ms));

    if let Some(token) = &config.auth_token {
        req = req.set("Authorization", &format!("Bearer {token}"));
    }

    let resp = req
        .set("Content-Type", "application/json")
        .send_string(&json)
        .map_err(|e| format!("push webhook request failed: {e}"))?;

    if resp.status() >= 200 && resp.status() < 300 {
        Ok(())
    } else {
        Err(format!("push webhook returned HTTP {}", resp.status()))
    }
}

fn log_push(payload: &PushPayload) -> Result<(), String> {
    let state_root = harmonia_config_store::get_config(COMPONENT, "global", "state-root")
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            env::temp_dir()
                .join("harmonia")
                .to_string_lossy()
                .to_string()
        });
    let log_path = harmonia_config_store::get_own(COMPONENT, "log")
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("{state_root}/push.log"));

    if let Some(parent) = std::path::Path::new(&log_path).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("push log dir create failed: {e}"))?;
    }

    let json = serde_json::to_string(payload)
        .map_err(|e| format!("push payload serialize failed: {e}"))?;

    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("push log open failed: {e}"))?;

    writeln!(f, "{json}").map_err(|e| format!("push log write failed: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes() {
        let p = PushPayload {
            device_token: "tok-123".into(),
            platform: "ios".into(),
            title: "Hello".into(),
            body: "World".into(),
            data: None,
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"platform\":\"ios\""));
        assert!(!json.contains("\"data\""));
    }

    #[test]
    fn payload_with_data_serializes() {
        let p = PushPayload {
            device_token: "tok-456".into(),
            platform: "android".into(),
            title: "Alert".into(),
            body: "Test".into(),
            data: Some("{\"silent\":true}".into()),
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"data\""));
    }

    #[test]
    fn log_mode_writes_file() {
        let dir = env::temp_dir().join("harmonia-push-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("push.log");

        let _ = harmonia_config_store::set_config(COMPONENT, "push-frontend", "mode", "log");
        let _ = harmonia_config_store::set_config(
            COMPONENT,
            "push-frontend",
            "log",
            log_path.to_str().unwrap(),
        );

        let config = PushConfig {
            webhook_url: String::new(),
            auth_token: None,
            timeout_ms: 5000,
        };
        let payload = PushPayload {
            device_token: "test".into(),
            platform: "ios".into(),
            title: "Test".into(),
            body: "Body".into(),
            data: None,
        };
        send_push(&config, &payload).unwrap();
        let content = fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("\"platform\":\"ios\""));

        // Config-store delete requires admin; in tests we just overwrite with empty or skip cleanup.
        let _ = harmonia_config_store::set_config(COMPONENT, "push-frontend", "mode", "");
        let _ = harmonia_config_store::set_config(COMPONENT, "push-frontend", "log", "");
        let _ = fs::remove_dir_all(&dir);
    }
}
