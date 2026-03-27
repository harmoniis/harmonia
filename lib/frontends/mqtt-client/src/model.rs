use harmonia_baseband_channel_protocol::CanonicalMobileEnvelope;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{OnceLock, RwLock};

pub(crate) const VERSION: &[u8] = b"harmonia-mqtt-client/0.3.0\0";
pub(crate) const COMPONENT: &str = "mqtt-frontend";
pub(crate) const BROKER_SCOPE: &str = "mqtt-broker";
pub(crate) const CONFIG_COMPONENT: &str = "harmonia-cli";
pub(crate) const DEFAULT_REMOTE_LABEL: &str = "mqtt-client-alice";
pub(crate) const PUSH_WEBHOOK_MUTATION: &str = r#"
mutation SendPush(
  $agentFingerprint: String!,
  $clientFingerprint: String!,
  $deviceId: String!,
  $publicKey: String!,
  $signature: String!,
  $title: String!,
  $body: String!,
  $data: String
) {
  send(
    input: {
      agentFingerprint: $agentFingerprint
      clientFingerprint: $clientFingerprint
      deviceId: $deviceId
      publicKey: $publicKey
      signature: $signature
      title: $title
      body: $body
      data: $data
    }
  ) {
    status
  }
}
"#;

pub(crate) type MessageEnvelope = CanonicalMobileEnvelope;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RemoteDeviceRecord {
    pub(crate) fingerprint: String,
    #[serde(rename = "deviceId")]
    pub(crate) device_id: String,
    pub(crate) label: Option<String>,
    pub(crate) platform: Option<String>,
    #[serde(rename = "pushToken")]
    pub(crate) push_token: Option<String>,
    #[serde(rename = "mqttIdentityFingerprint")]
    pub(crate) mqtt_identity_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DeviceInfo {
    pub(crate) device_id: String,
    #[serde(default)]
    pub(crate) owner_fingerprint: String,
    pub(crate) platform: String,
    pub(crate) platform_version: String,
    pub(crate) app_version: String,
    #[serde(default)]
    pub(crate) device_model: String,
    #[serde(default)]
    pub(crate) capabilities: Vec<String>,
    #[serde(default)]
    pub(crate) permissions_granted: Vec<String>,
    #[serde(default)]
    pub(crate) a2ui_version: String,
    #[serde(default)]
    pub(crate) push_token: Option<String>,
    #[serde(default)]
    pub(crate) mqtt_identity_fingerprint: Option<String>,
    #[serde(skip)]
    pub(crate) connected: bool,
    #[serde(skip)]
    pub(crate) last_seen_ms: u64,
}

impl DeviceInfo {
    /// Render device info as an s-expression metadata string.
    /// This is the per-message metadata emitted in the 3rd poll field.
    pub(crate) fn to_metadata_sexp(&self) -> String {
        let caps = if self.capabilities.is_empty() {
            "nil".to_string()
        } else {
            let items: Vec<String> = self
                .capabilities
                .iter()
                .map(|c| format!("\"{}\"", c))
                .collect();
            format!("({})", items.join(" "))
        };
        format!(
            "(:platform \"{}\" :device-id \"{}\" :device-model \"{}\" :a2ui-version \"{}\" :capabilities {} :app-version \"{}\")",
            self.platform, self.device_id, self.device_model,
            self.a2ui_version, caps, self.app_version,
        )
    }
}

#[derive(Debug)]
pub(crate) struct VaultSignResult {
    pub(crate) public_key: String,
    pub(crate) signature: String,
}

pub(crate) type InboundMessage = (String, String, Option<String>);

pub(crate) static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
pub(crate) static FRONTEND_VERSION: &[u8] = b"harmonia-mqtt-frontend/0.3.0\0";
pub(crate) static SUBSCRIBED_TOPICS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
pub(crate) static INBOUND_QUEUE: OnceLock<RwLock<VecDeque<InboundMessage>>> = OnceLock::new();

pub(crate) fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub(crate) fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

pub(crate) fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

pub(crate) fn subscribed_topics() -> &'static RwLock<Vec<String>> {
    SUBSCRIBED_TOPICS.get_or_init(|| RwLock::new(Vec::new()))
}

pub(crate) fn inbound_queue() -> &'static RwLock<VecDeque<InboundMessage>> {
    INBOUND_QUEUE.get_or_init(|| RwLock::new(VecDeque::new()))
}
