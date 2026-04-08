use rumqttc::{Event, Incoming, QoS};
use std::os::raw::c_char;
use std::time::Duration;

use crate::api::harmonia_mqtt_client_publish;
use crate::connection::connect;
use crate::device::{
    build_envelope_metadata, device_registry, extract_device_id_from_topic, handle_device_connect,
    handle_device_disconnect, is_device_connect_topic, is_device_disconnect_topic,
    load_remote_device_registry, merge_metadata_sexp, origin_is_trusted,
    validate_agent_fingerprint,
};
use crate::ffi::{cstr_to_string, harmonia_mqtt_client_free_string};
use crate::model::{
    clear_error, inbound_queue, set_error, subscribed_topics, MessageEnvelope, FRONTEND_VERSION,
};
use crate::publish::{extract_sexp_value, send_offline_push};
use crate::queue::{enqueue_offline_message, load_offline_queue};

pub fn harmonia_frontend_version() -> *const c_char {
    FRONTEND_VERSION.as_ptr().cast()
}

pub fn harmonia_frontend_healthcheck() -> i32 {
    1
}

pub fn harmonia_frontend_init(config: *const c_char) -> i32 {
    let config = match cstr_to_string(config) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    load_offline_queue();
    load_remote_device_registry();

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
    let push_url = extract_sexp_value(&config, "push-webhook-url").or_else(|| {
        harmonia_config_store::get_own(crate::model::COMPONENT, "push-webhook-url")
            .ok()
            .flatten()
    });
    if let Some(url) = push_url {
        if !url.is_empty() {
            let token = extract_sexp_value(&config, "push-webhook-token").or_else(|| {
                harmonia_config_store::get_own(crate::model::COMPONENT, "push-webhook-token")
                    .ok()
                    .flatten()
            });
            let timeout = extract_sexp_value(&config, "push-webhook-timeout-ms")
                .and_then(|v| v.parse::<u64>().ok())
                .or_else(|| {
                    harmonia_config_store::get_own(
                        crate::model::COMPONENT,
                        "push-webhook-timeout-ms",
                    )
                    .ok()
                    .flatten()
                    .and_then(|v| v.parse::<u64>().ok())
                })
                .unwrap_or(5000);
            if let Ok(mut cfg) = crate::device::push_config().write() {
                *cfg = Some(harmonia_push::PushConfig {
                    webhook_url: url,
                    auth_token: token.filter(|value| !value.is_empty()),
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
                                    let origin_trusted = origin_is_trusted(&env);
                                    let fp_valid =
                                        validate_agent_fingerprint(&env) && origin_trusted;
                                    let meta =
                                        build_envelope_metadata(&env, fp_valid, origin_trusted);
                                    (payload.clone(), Some(meta))
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

pub fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        set_error("null buffer");
        return -1;
    }
    let messages: Vec<_> = if let Ok(mut q) = inbound_queue().write() {
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

pub fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    load_remote_device_registry();
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
                let _ = enqueue_offline_message(&device_id, &topic, &payload_str);
                send_offline_push(device, &payload_str);
                clear_error();
                return 0;
            }
        }
    }

    // Device is online (or no device context): publish normally
    harmonia_mqtt_client_publish(channel, payload)
}

pub fn harmonia_frontend_last_error() -> *const c_char {
    crate::api::harmonia_mqtt_client_last_error() as *const c_char
}

pub fn harmonia_frontend_shutdown() -> i32 {
    if let Ok(mut t) = subscribed_topics().write() {
        t.clear();
    }
    if let Ok(mut q) = inbound_queue().write() {
        q.clear();
    }
    if let Ok(mut reg) = device_registry().write() {
        reg.clear();
    }
    if let Ok(mut cfg) = crate::device::push_config().write() {
        *cfg = None;
    }
    clear_error();
    0
}

pub fn harmonia_frontend_free_string(ptr: *mut c_char) {
    harmonia_mqtt_client_free_string(ptr)
}
