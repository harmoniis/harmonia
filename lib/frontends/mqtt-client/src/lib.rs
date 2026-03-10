use chrono::Utc;
use rumqttc::{Client, Event, Incoming, MqttOptions, Outgoing, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::process;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const VERSION: &[u8] = b"harmonia-mqtt-client/0.3.0\0";
const COMPONENT: &str = "mqtt-frontend";
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

#[derive(Debug, Serialize, Deserialize)]
struct MessageEnvelope {
    v: u8,
    kind: String,
    #[serde(rename = "type")]
    type_name: String,
    id: String,
    ts: String,
    agent_fp: String,
    client_fp: String,
    body: serde_json::Value,
}

// ─── Device Registry ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceInfo {
    device_id: String,
    platform: String,
    platform_version: String,
    app_version: String,
    #[serde(default)]
    device_model: String,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    permissions_granted: Vec<String>,
    #[serde(default)]
    a2ui_version: String,
    #[serde(default)]
    push_token: Option<String>,
    #[serde(skip)]
    connected: bool,
    #[serde(skip)]
    last_seen_ms: u64,
}

impl DeviceInfo {
    /// Render device info as an s-expression metadata string.
    /// This is the per-message metadata emitted in the 3rd poll field.
    fn to_metadata_sexp(&self) -> String {
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

fn device_registry() -> &'static RwLock<HashMap<String, DeviceInfo>> {
    static REG: OnceLock<RwLock<HashMap<String, DeviceInfo>>> = OnceLock::new();
    REG.get_or_init(|| RwLock::new(HashMap::new()))
}

fn push_config() -> &'static RwLock<Option<harmonia_push::PushConfig>> {
    static CFG: OnceLock<RwLock<Option<harmonia_push::PushConfig>>> = OnceLock::new();
    CFG.get_or_init(|| RwLock::new(None))
}

fn offline_queue() -> &'static RwLock<HashMap<String, VecDeque<(String, String)>>> {
    static Q: OnceLock<RwLock<HashMap<String, VecDeque<(String, String)>>>> = OnceLock::new();
    Q.get_or_init(|| RwLock::new(HashMap::new()))
}

// ─── Helpers ────────────────────────────────────────────────────────

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn parse_broker() -> Result<(String, u16), String> {
    let raw =
        harmonia_config_store::get_own_or(COMPONENT, "broker", "test.mosquitto.org:1883").unwrap_or_else(|_| "test.mosquitto.org:1883".to_string());
    let (host, port_raw) = raw
        .split_once(':')
        .ok_or_else(|| format!("invalid HARMONIA_MQTT_BROKER: {raw}"))?;
    let port = port_raw
        .parse::<u16>()
        .map_err(|e| format!("invalid mqtt port: {e}"))?;
    Ok((host.to_string(), port))
}

fn timeout_ms() -> u64 {
    harmonia_config_store::get_own(COMPONENT, "timeout-ms")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5000)
}

fn tls_enabled() -> bool {
    harmonia_config_store::get_own(COMPONENT, "tls")
        .ok()
        .flatten()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn client_id(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("harmonia-{prefix}-{}-{ts}", process::id())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn connect(prefix: &str) -> Result<(Client, rumqttc::Connection), String> {
    let (host, port) = parse_broker()?;
    let mut opts = MqttOptions::new(client_id(prefix), host, port);
    opts.set_keep_alive(Duration::from_secs(5));
    if tls_enabled() {
        let ca_path = harmonia_config_store::get_own(COMPONENT, "ca-cert")
            .ok()
            .flatten()
            .ok_or_else(|| "HARMONIA_MQTT_CA_CERT required when HARMONIA_MQTT_TLS=1".to_string())?;
        let ca = fs::read(&ca_path).map_err(|e| format!("read ca cert failed: {e}"))?;
        let _ = harmonia_vault::init_from_env();
        // Keep a recoverable deterministic seed for MQTT TLS lineage.
        if let Ok(seed_hex) = harmonia_vault::derive_component_seed_hex("mqtt-frontend", "tls") {
            let _ = harmonia_vault::set_secret_for_symbol("mqtt_tls_master_seed", &seed_hex);
        }

        let client_auth = match (
            harmonia_config_store::get_own(COMPONENT, "client-cert").ok().flatten(),
            harmonia_config_store::get_own(COMPONENT, "client-key").ok().flatten(),
        ) {
            (Some(cert_path), Some(key_path)) => {
                let cert =
                    fs::read(&cert_path).map_err(|e| format!("read client cert failed: {e}"))?;
                let key =
                    fs::read(&key_path).map_err(|e| format!("read client key failed: {e}"))?;
                if let Ok(cert_pem) = String::from_utf8(cert.clone()) {
                    let _ = harmonia_vault::set_secret_for_symbol(
                        "mqtt_tls_client_cert_pem",
                        &cert_pem,
                    );
                }
                if let Ok(key_pem) = String::from_utf8(key.clone()) {
                    let _ =
                        harmonia_vault::set_secret_for_symbol("mqtt_tls_client_key_pem", &key_pem);
                }
                Some((cert, key))
            }
            _ => {
                let cert = harmonia_vault::get_secret_for_component(
                    "mqtt-frontend",
                    "mqtt_tls_client_cert_pem",
                )
                .ok()
                .flatten();
                let key = harmonia_vault::get_secret_for_component(
                    "mqtt-frontend",
                    "mqtt_tls_client_key_pem",
                )
                .ok()
                .flatten();
                match (cert, key) {
                    (Some(c), Some(k)) => Some((c.into_bytes(), k.into_bytes())),
                    _ => None,
                }
            }
        };
        opts.set_transport(Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth,
        }));
    }
    Ok(Client::new(opts, 10))
}

/// Extract device_id from an MQTT topic path.
/// Pattern: `harmonia/{agent_id}/device/{device_id}/...`
fn extract_device_id_from_topic(topic: &str) -> Option<String> {
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
fn is_device_connect_topic(topic: &str) -> bool {
    topic.ends_with("/connect") && topic.contains("/device/")
}

/// Check if a topic is a device disconnect event.
fn is_device_disconnect_topic(topic: &str) -> bool {
    topic.ends_with("/disconnect") && topic.contains("/device/")
}

/// Wave 4.1: Validate agent_fp against vault-stored expected fingerprint.
/// Returns true if fingerprint is valid or no expected fingerprint is configured.
fn validate_agent_fingerprint(envelope: &MessageEnvelope) -> bool {
    let _ = harmonia_vault::init_from_env();
    let expected_fp =
        match harmonia_vault::get_secret_for_component("mqtt-frontend", "mqtt_agent_fp")
            .ok()
            .flatten()
        {
            Some(fp) if !fp.is_empty() => fp,
            _ => return true, // No expected fingerprint configured, allow
        };
    if envelope.agent_fp.is_empty() {
        log::warn!("mqtt: message missing agent_fp, downgrading to untrusted");
        return false;
    }
    if envelope.agent_fp != expected_fp {
        log::warn!(
            "mqtt: agent_fp mismatch (got {}, expected {}), downgrading to untrusted",
            envelope.agent_fp,
            expected_fp
        );
        return false;
    }
    true
}

fn extract_envelope_payload_text(env: &MessageEnvelope) -> String {
    if let Some(text) = env.body.get("text").and_then(|v| v.as_str()) {
        return text.to_string();
    }
    if let Some(payload) = env.body.get("payload").and_then(|v| v.as_str()) {
        return payload.to_string();
    }
    env.body.to_string()
}

fn build_envelope_metadata(env: &MessageEnvelope, fp_valid: bool) -> String {
    let security = if fp_valid {
        "authenticated"
    } else {
        "untrusted"
    };
    format!(
        "(:origin-fp \"{}\" :agent-fp \"{}\" :security \"{}\")",
        env.client_fp, env.agent_fp, security
    )
}

fn merge_metadata_sexp(a: Option<&str>, b: Option<&str>) -> Option<String> {
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
fn handle_device_connect(payload: &str) {
    let mut device: DeviceInfo = match serde_json::from_str(payload) {
        Ok(d) => d,
        Err(_) => return,
    };
    device.connected = true;
    device.last_seen_ms = now_ms();
    let device_id = device.device_id.clone();

    if let Ok(mut reg) = device_registry().write() {
        reg.insert(device_id.clone(), device);
    }

    // Flush any offline-queued messages for this device
    flush_offline_queue(&device_id);
}

/// Handle device disconnect: mark as not connected.
fn handle_device_disconnect(topic: &str) {
    if let Some(device_id) = extract_device_id_from_topic(topic) {
        if let Ok(mut reg) = device_registry().write() {
            if let Some(device) = reg.get_mut(&device_id) {
                device.connected = false;
                device.last_seen_ms = now_ms();
            }
        }
    }
}

/// Flush offline-queued messages for a device via MQTT publish.
fn flush_offline_queue(device_id: &str) {
    let messages: Vec<(String, String)> = if let Ok(mut q) = offline_queue().write() {
        q.remove(device_id)
            .map(|dq| dq.into_iter().collect())
            .unwrap_or_default()
    } else {
        return;
    };

    if messages.is_empty() {
        return;
    }

    let (client, mut connection) = match connect("flush") {
        Ok(v) => v,
        Err(_) => return,
    };
    for (topic, payload) in &messages {
        let _ = client.publish(topic.clone(), QoS::AtLeastOnce, false, payload.as_bytes());
    }
    // Drain connection events briefly to ensure delivery
    let deadline = Instant::now() + Duration::from_millis(2000);
    for event in connection.iter() {
        match event {
            Ok(_) => {}
            Err(_) => break,
        }
        if Instant::now() > deadline {
            break;
        }
    }
}

/// Send a push notification for an offline device.
fn send_offline_push(device: &DeviceInfo, payload: &str) {
    let push_token = match &device.push_token {
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

    let push_payload = harmonia_push::PushPayload {
        device_token: push_token,
        platform: device.platform.clone(),
        title: "Harmonia".to_string(),
        body: truncate_for_push(payload),
        data: None,
    };

    let _ = harmonia_push::send_push(&config, &push_payload);
}

fn truncate_for_push(text: &str) -> String {
    if text.len() <= 256 {
        text.to_string()
    } else {
        format!("{}...", &text[..253])
    }
}

// ─── Config sexp parser ─────────────────────────────────────────────

fn extract_sexp_value(sexp: &str, key: &str) -> Option<String> {
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

// ─── Legacy MQTT Client API ─────────────────────────────────────────

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_publish(
    topic: *const c_char,
    payload: *const c_char,
) -> i32 {
    let topic = match cstr_to_string(topic) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let payload = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let (client, mut connection) = match connect("pub") {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    if let Err(e) = client.publish(topic, QoS::AtLeastOnce, true, payload.into_bytes()) {
        set_error(format!("mqtt publish failed: {e}"));
        return -1;
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms());
    for event in connection.iter() {
        match event {
            Ok(Event::Outgoing(Outgoing::Publish(_)))
            | Ok(Event::Incoming(Incoming::PubAck(_))) => {
                clear_error();
                return 0;
            }
            Ok(_) => {}
            Err(e) => {
                set_error(format!("mqtt connection error: {e}"));
                return -1;
            }
        }
        if Instant::now() > deadline {
            break;
        }
    }
    set_error("mqtt publish timeout");
    -1
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_poll(topic: *const c_char) -> *mut c_char {
    let topic = match cstr_to_string(topic) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let (client, mut connection) = match connect("poll") {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    if let Err(e) = client.subscribe(topic.clone(), QoS::AtLeastOnce) {
        set_error(format!("mqtt subscribe failed: {e}"));
        return std::ptr::null_mut();
    }
    let deadline = Instant::now() + Duration::from_millis(timeout_ms());
    for event in connection.iter() {
        match event {
            Ok(Event::Incoming(Incoming::Publish(p))) if p.topic == topic => {
                clear_error();
                let payload = String::from_utf8_lossy(&p.payload).to_string();
                return to_c_string(payload);
            }
            Ok(_) => {}
            Err(e) => {
                set_error(format!("mqtt poll failed: {e}"));
                return std::ptr::null_mut();
            }
        }
        if Instant::now() > deadline {
            break;
        }
    }
    set_error(format!("mqtt timeout waiting for topic: {topic}"));
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_reset() -> i32 {
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "mqtt lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_make_envelope(
    kind: *const c_char,
    type_name: *const c_char,
    agent_fp: *const c_char,
    client_fp: *const c_char,
    body_json: *const c_char,
) -> *mut c_char {
    let kind = match cstr_to_string(kind) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let type_name = match cstr_to_string(type_name) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let agent_fp = match cstr_to_string(agent_fp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let client_fp = match cstr_to_string(client_fp) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let body_json = match cstr_to_string(body_json) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let body: serde_json::Value = match serde_json::from_str(&body_json) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("invalid envelope body json: {e}"));
            return std::ptr::null_mut();
        }
    };
    let env = MessageEnvelope {
        v: 1,
        kind,
        type_name,
        id: Uuid::new_v4().to_string(),
        ts: Utc::now().to_rfc3339(),
        agent_fp,
        client_fp,
        body,
    };
    match serde_json::to_string(&env) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(format!("envelope serialize failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_parse_envelope(payload: *const c_char) -> *mut c_char {
    let payload = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let env: MessageEnvelope = match serde_json::from_str(&payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(format!("invalid envelope json: {e}"));
            return std::ptr::null_mut();
        }
    };
    if env.v != 1 {
        set_error(format!("unsupported envelope version {}", env.v));
        return std::ptr::null_mut();
    }
    match serde_json::to_string(&env) {
        Ok(v) => {
            clear_error();
            to_c_string(v)
        }
        Err(e) => {
            set_error(format!("envelope normalize failed: {e}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_mqtt_client_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr)) };
}

// ─── Frontend FFI Contract ──────────────────────────────────────────
// These 8 symbols allow the gateway to hot-load mqtt-client as a frontend .so.

static FRONTEND_VERSION: &[u8] = b"harmonia-mqtt-frontend/0.3.0\0";
static SUBSCRIBED_TOPICS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();
type InboundMessage = (String, String, Option<String>);
static INBOUND_QUEUE: OnceLock<RwLock<VecDeque<InboundMessage>>> = OnceLock::new();

fn subscribed_topics() -> &'static RwLock<Vec<String>> {
    SUBSCRIBED_TOPICS.get_or_init(|| RwLock::new(Vec::new()))
}
fn inbound_queue() -> &'static RwLock<VecDeque<InboundMessage>> {
    INBOUND_QUEUE.get_or_init(|| RwLock::new(VecDeque::new()))
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_version() -> *const c_char {
    FRONTEND_VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_init(config: *const c_char) -> i32 {
    let config = match cstr_to_string(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    // Parse subscribed topics from config
    if let Some(topics_str) = extract_sexp_value(&config, "topics") {
        let topics: Vec<String> = topics_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if let Ok(mut t) = subscribed_topics().write() {
            *t = topics;
        }
    }

    // Parse push webhook config
    if let Some(url) = extract_sexp_value(&config, "push-webhook-url") {
        if !url.is_empty() {
            let token = extract_sexp_value(&config, "push-webhook-token");
            let timeout = extract_sexp_value(&config, "push-webhook-timeout-ms")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(5000);
            if let Ok(mut cfg) = push_config().write() {
                *cfg = Some(harmonia_push::PushConfig {
                    webhook_url: url,
                    auth_token: if token.as_deref() == Some("") {
                        None
                    } else {
                        token
                    },
                    timeout_ms: timeout,
                });
            }
        }
    }

    // Spawn background poll threads for subscribed topics
    let topics = subscribed_topics()
        .read()
        .map(|t| t.clone())
        .unwrap_or_default();
    for topic in topics {
        let topic_clone = topic.clone();
        std::thread::spawn(move || loop {
            let (client, mut connection) = match connect("frontend-poll") {
                Ok(v) => v,
                Err(_) => {
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };
            let _ = client.subscribe(&topic_clone, QoS::AtLeastOnce);
            for event in connection.iter() {
                match event {
                    Ok(Event::Incoming(Incoming::Publish(p))) => {
                        let payload = String::from_utf8_lossy(&p.payload).to_string();

                        // Handle device connect/disconnect events
                        if is_device_connect_topic(&p.topic) {
                            handle_device_connect(&payload);
                            continue;
                        }
                        if is_device_disconnect_topic(&p.topic) {
                            handle_device_disconnect(&p.topic);
                            continue;
                        }

                        let (effective_payload, envelope_meta) =
                            match serde_json::from_str::<MessageEnvelope>(&payload) {
                                Ok(env) => {
                                    let fp_valid = validate_agent_fingerprint(&env);
                                    let meta = build_envelope_metadata(&env, fp_valid);
                                    (extract_envelope_payload_text(&env), Some(meta))
                                }
                                Err(_) => (payload, None),
                            };

                        if let Ok(mut q) = inbound_queue().write() {
                            q.push_back((p.topic.clone(), effective_payload, envelope_meta));
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        });
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        set_error("null buffer");
        return -1;
    }
    let messages: Vec<InboundMessage> = if let Ok(mut q) = inbound_queue().write() {
        q.drain(..).collect()
    } else {
        set_error("mqtt inbound lock poisoned");
        return -1;
    };
    if messages.is_empty() {
        return 0;
    }
    let mut output = String::new();
    for (topic, payload, envelope_meta) in &messages {
        output.push_str(topic);
        output.push('\t');
        output.push_str(payload);
        // Emit device metadata as third tab-field when available
        let mut metadata = envelope_meta.clone();
        if let Some(device_id) = extract_device_id_from_topic(topic) {
            if let Ok(reg) = device_registry().read() {
                if let Some(device) = reg.get(&device_id) {
                    metadata =
                        merge_metadata_sexp(metadata.as_deref(), Some(&device.to_metadata_sexp()));
                }
            }
        }
        if let Some(meta) = metadata {
            output.push('\t');
            output.push_str(&meta);
        }
        output.push('\n');
    }
    let bytes = output.as_bytes();
    let write_len = bytes.len().min(buf_len - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, write_len);
        *((buf as *mut u8).add(write_len)) = 0;
    }
    write_len as i32
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let topic = match cstr_to_string(channel) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let payload_str = match cstr_to_string(payload) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    // Check if this is targeted at a specific device and whether it's offline
    if let Some(device_id) = extract_device_id_from_topic(&topic) {
        let device_info = device_registry()
            .read()
            .ok()
            .and_then(|reg| reg.get(&device_id).cloned());

        if let Some(ref device) = device_info {
            if !device.connected {
                // Device is offline: queue the message and send push notification
                if let Ok(mut q) = offline_queue().write() {
                    q.entry(device_id.clone())
                        .or_default()
                        .push_back((topic.clone(), payload_str.clone()));
                }
                send_offline_push(device, &payload_str);
                clear_error();
                return 0;
            }
        }
    }

    // Device is online (or no device context): publish normally
    harmonia_mqtt_client_publish(channel, payload)
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_last_error() -> *const c_char {
    harmonia_mqtt_client_last_error() as *const c_char
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_shutdown() -> i32 {
    if let Ok(mut t) = subscribed_topics().write() {
        t.clear();
    }
    if let Ok(mut q) = inbound_queue().write() {
        q.clear();
    }
    if let Ok(mut reg) = device_registry().write() {
        reg.clear();
    }
    if let Ok(mut q) = offline_queue().write() {
        q.clear();
    }
    if let Ok(mut cfg) = push_config().write() {
        *cfg = None;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_frontend_free_string(ptr: *mut c_char) {
    harmonia_mqtt_client_free_string(ptr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_mqtt_client_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_mqtt_client_version().is_null());
    }

    #[test]
    fn envelope_roundtrip_v1() {
        let env = MessageEnvelope {
            v: 1,
            kind: "cmd".to_string(),
            type_name: "text_input".to_string(),
            id: "1".to_string(),
            ts: "2026-01-01T00:00:00Z".to_string(),
            agent_fp: "A".to_string(),
            client_fp: "C".to_string(),
            body: serde_json::json!({"text":"hi"}),
        };
        let json = serde_json::to_string(&env).expect("serialize");
        let parsed: MessageEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.kind, "cmd");
    }

    #[test]
    fn device_info_metadata_sexp() {
        let d = DeviceInfo {
            device_id: "dev-1".into(),
            platform: "ios".into(),
            platform_version: "17.2".into(),
            app_version: "1.0.0".into(),
            device_model: "iPhone 15".into(),
            capabilities: vec!["voice".into(), "location".into()],
            permissions_granted: vec![],
            a2ui_version: "1.0".into(),
            push_token: None,
            connected: true,
            last_seen_ms: 0,
        };
        let sexp = d.to_metadata_sexp();
        assert!(sexp.contains(":platform \"ios\""));
        assert!(sexp.contains(":a2ui-version \"1.0\""));
        assert!(sexp.contains("\"voice\" \"location\""));
    }

    #[test]
    fn extract_device_id_works() {
        assert_eq!(
            extract_device_id_from_topic("harmonia/agent1/device/uuid-123/msg"),
            Some("uuid-123".to_string())
        );
        assert_eq!(extract_device_id_from_topic("some/other/topic"), None);
    }

    #[test]
    fn connect_disconnect_topic_detection() {
        assert!(is_device_connect_topic("harmonia/a1/device/d1/connect"));
        assert!(!is_device_connect_topic("harmonia/a1/device/d1/msg"));
        assert!(is_device_disconnect_topic(
            "harmonia/a1/device/d1/disconnect"
        ));
    }
}
