use rumqttc::{Client, Event, Incoming, MqttOptions, Outgoing, QoS, TlsConfiguration, Transport};
use serde::{Deserialize, Serialize};
use std::env;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::process;
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use chrono::Utc;

const VERSION: &[u8] = b"harmonia-mqtt-client/0.2.0\0";
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
    // Safety: caller provides valid null-terminated string.
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
        env::var("HARMONIA_MQTT_BROKER").unwrap_or_else(|_| "test.mosquitto.org:1883".to_string());
    let (host, port_raw) = raw
        .split_once(':')
        .ok_or_else(|| format!("invalid HARMONIA_MQTT_BROKER: {raw}"))?;
    let port = port_raw
        .parse::<u16>()
        .map_err(|e| format!("invalid mqtt port: {e}"))?;
    Ok((host.to_string(), port))
}

fn timeout_ms() -> u64 {
    env::var("HARMONIA_MQTT_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5000)
}

fn tls_enabled() -> bool {
    env::var("HARMONIA_MQTT_TLS")
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

fn connect(prefix: &str) -> Result<(Client, rumqttc::Connection), String> {
    let (host, port) = parse_broker()?;
    let mut opts = MqttOptions::new(client_id(prefix), host, port);
    opts.set_keep_alive(Duration::from_secs(5));
    if tls_enabled() {
        let ca_path = env::var("HARMONIA_MQTT_CA_CERT")
            .map_err(|_| "HARMONIA_MQTT_CA_CERT required when HARMONIA_MQTT_TLS=1".to_string())?;
        let ca = fs::read(&ca_path).map_err(|e| format!("read ca cert failed: {e}"))?;
        let client_auth = match (
            env::var("HARMONIA_MQTT_CLIENT_CERT"),
            env::var("HARMONIA_MQTT_CLIENT_KEY"),
        ) {
            (Ok(cert_path), Ok(key_path)) => {
                let cert =
                    fs::read(cert_path).map_err(|e| format!("read client cert failed: {e}"))?;
                let key = fs::read(key_path).map_err(|e| format!("read client key failed: {e}"))?;
                Some((cert, key))
            }
            _ => None,
        };
        opts.set_transport(Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth,
        }));
    }
    Ok(Client::new(opts, 10))
}

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
    // Safety: ptr must come from CString::into_raw from this crate.
    unsafe { drop(CString::from_raw(ptr)) };
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
}
