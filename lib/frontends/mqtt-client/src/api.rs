use rumqttc::{Event, Incoming, Outgoing, QoS};
use std::os::raw::c_char;
use std::time::{Duration, Instant};

use crate::connection::{connect, timeout_ms};
use crate::ffi::{cstr_to_string, to_c_string};
use crate::model::{clear_error, last_error, set_error, VERSION};

// Re-export everything from sub-modules so the public API surface stays identical.
pub use crate::envelope::{harmonia_mqtt_client_make_envelope, harmonia_mqtt_client_parse_envelope};
pub use crate::ffi::harmonia_mqtt_client_free_string;
pub use crate::frontend::{
    harmonia_frontend_free_string, harmonia_frontend_healthcheck, harmonia_frontend_init,
    harmonia_frontend_last_error, harmonia_frontend_poll, harmonia_frontend_send,
    harmonia_frontend_shutdown, harmonia_frontend_version,
};

// ---- Legacy MQTT Client API ----

pub fn harmonia_mqtt_client_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_mqtt_client_healthcheck() -> i32 {
    1
}

pub fn harmonia_mqtt_client_publish(topic: *const c_char, payload: *const c_char) -> i32 {
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

pub fn harmonia_mqtt_client_poll(topic: *const c_char) -> *mut c_char {
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

pub fn harmonia_mqtt_client_reset() -> i32 {
    clear_error();
    0
}

pub fn harmonia_mqtt_client_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "mqtt lock poisoned".to_string());
    to_c_string(msg)
}
