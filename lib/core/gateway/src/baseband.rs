use crate::model::{
    AuditContext, CanonicalMobileEnvelope, ChannelBatch, ChannelBody, ChannelEnvelope, ChannelRef,
    ConversationRef, PeerRef, SecurityContext, SecurityLabel, TransportContext,
};
use crate::registry::Registry;
use harmonia_signal_integrity::{
    compute_dissonance as compute_dissonance_score, scan_for_injection,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

fn compute_dissonance(payload: &str) -> f64 {
    let report = scan_for_injection(payload);
    compute_dissonance_score(&report)
}

static ENVELOPE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_envelope_id() -> u64 {
    ENVELOPE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn parse_poll_line(line: &str) -> (String, String, Option<String>) {
    let parts: Vec<&str> = line.splitn(3, '\t').collect();
    match parts.len() {
        3 => (
            parts[0].to_string(),
            parts[1].to_string(),
            Some(parts[2].to_string()),
        ),
        2 => (parts[0].to_string(), parts[1].to_string(), None),
        _ => (String::new(), parts[0].to_string(), None),
    }
}

fn metadata_string_value(metadata: Option<&str>, key: &str) -> Option<String> {
    let meta = metadata?;
    let needle = format!(":{} \"", key);
    let start = meta.find(&needle)?;
    let from = start + needle.len();
    let rest = &meta[from..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn metadata_bool_value(metadata: Option<&str>, key: &str) -> Option<bool> {
    let meta = metadata?;
    let t_needle = format!(":{} t", key);
    let nil_needle = format!(":{} nil", key);
    if meta.contains(&t_needle) {
        Some(true)
    } else if meta.contains(&nil_needle) {
        Some(false)
    } else {
        None
    }
}

fn generic_type_name(payload: &str) -> &'static str {
    let trimmed = payload.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        "message.structured"
    } else {
        "message.text"
    }
}

fn generic_body(payload: &str) -> ChannelBody {
    if payload.trim_start().starts_with('{') || payload.trim_start().starts_with('[') {
        ChannelBody {
            format: "json".to_string(),
            text: payload.to_string(),
            raw: payload.to_string(),
        }
    } else {
        ChannelBody::text(payload)
    }
}

fn build_generic_envelope(
    driver_name: &str,
    security: SecurityLabel,
    capabilities: &[crate::model::Capability],
    address: &str,
    payload: &str,
    metadata: Option<String>,
) -> ChannelEnvelope {
    let fingerprint_valid =
        metadata_bool_value(metadata.as_deref(), "fingerprint-valid").unwrap_or(true);
    let origin_fp = metadata_string_value(metadata.as_deref(), "origin-fp");
    let peer_id = metadata_string_value(metadata.as_deref(), "device-id")
        .or_else(|| origin_fp.clone())
        .unwrap_or_else(|| {
            if address.is_empty() {
                driver_name.to_string()
            } else {
                address.to_string()
            }
        });
    let mut peer = PeerRef::new(peer_id);
    peer.origin_fp = origin_fp;
    peer.agent_fp = metadata_string_value(metadata.as_deref(), "agent-fp");
    peer.device_id = metadata_string_value(metadata.as_deref(), "device-id");
    peer.platform = metadata_string_value(metadata.as_deref(), "platform");
    peer.device_model = metadata_string_value(metadata.as_deref(), "device-model");
    peer.app_version = metadata_string_value(metadata.as_deref(), "app-version");
    peer.a2ui_version = metadata_string_value(metadata.as_deref(), "a2ui-version");

    let label = if fingerprint_valid {
        security
    } else {
        SecurityLabel::Untrusted
    };
    let channel = ChannelRef::new(driver_name, address);
    let body = generic_body(payload);
    let body_text = body.text.clone();
    ChannelEnvelope {
        id: next_envelope_id(),
        version: 1,
        kind: "external".to_string(),
        type_name: generic_type_name(payload).to_string(),
        conversation: ConversationRef::new(channel.label.clone()),
        channel: channel.clone(),
        peer,
        body,
        capabilities: capabilities.to_vec(),
        security: SecurityContext {
            label,
            source: "gateway".to_string(),
            fingerprint_valid,
        },
        audit: AuditContext {
            timestamp_ms: now_ms(),
            dissonance: compute_dissonance(&body_text),
        },
        attachments: Vec::new(),
        transport: TransportContext {
            kind: driver_name.to_string(),
            raw_address: address.to_string(),
            raw_metadata: metadata,
        },
    }
}

fn build_mqtt_envelope(
    security: SecurityLabel,
    capabilities: &[crate::model::Capability],
    topic: &str,
    payload: &str,
    metadata: Option<String>,
) -> ChannelEnvelope {
    let now = now_ms();
    match serde_json::from_str::<CanonicalMobileEnvelope>(payload) {
        Ok(envelope) => {
            let fingerprint_valid =
                metadata_bool_value(metadata.as_deref(), "fingerprint-valid").unwrap_or(true);
            let mut peer = PeerRef::new(
                metadata_string_value(metadata.as_deref(), "device-id").unwrap_or_else(|| {
                    if envelope.client_fp.is_empty() {
                        topic.to_string()
                    } else {
                        envelope.client_fp.clone()
                    }
                }),
            );
            peer.origin_fp = if envelope.client_fp.is_empty() {
                metadata_string_value(metadata.as_deref(), "origin-fp")
            } else {
                Some(envelope.client_fp.clone())
            };
            peer.agent_fp = if envelope.agent_fp.is_empty() {
                metadata_string_value(metadata.as_deref(), "agent-fp")
            } else {
                Some(envelope.agent_fp.clone())
            };
            peer.device_id = metadata_string_value(metadata.as_deref(), "device-id");
            peer.platform = metadata_string_value(metadata.as_deref(), "platform");
            peer.device_model = metadata_string_value(metadata.as_deref(), "device-model");
            peer.app_version = metadata_string_value(metadata.as_deref(), "app-version");
            peer.a2ui_version = metadata_string_value(metadata.as_deref(), "a2ui-version");

            let body_text = envelope.body_text();
            let body = ChannelBody {
                format: envelope.body_format().to_string(),
                text: body_text.clone(),
                raw: envelope.body.to_string(),
            };
            let label = if fingerprint_valid {
                security
            } else {
                SecurityLabel::Untrusted
            };
            let channel = ChannelRef::new("mqtt", topic);
            ChannelEnvelope {
                id: next_envelope_id(),
                version: envelope.v,
                kind: envelope.kind,
                type_name: envelope.type_name,
                conversation: ConversationRef::new(channel.label.clone()),
                channel: channel.clone(),
                peer,
                body,
                capabilities: capabilities.to_vec(),
                security: SecurityContext {
                    label,
                    source: "mqtt-envelope".to_string(),
                    fingerprint_valid,
                },
                audit: AuditContext {
                    timestamp_ms: now,
                    dissonance: compute_dissonance(&body_text),
                },
                attachments: Vec::new(),
                transport: TransportContext {
                    kind: "mqtt".to_string(),
                    raw_address: topic.to_string(),
                    raw_metadata: metadata,
                },
            }
        }
        Err(_) => build_generic_envelope("mqtt", security, capabilities, topic, payload, metadata),
    }
}

fn parse_frontend_envelopes(
    driver_name: &str,
    security: SecurityLabel,
    capabilities: &[crate::model::Capability],
    raw: &str,
) -> Vec<ChannelEnvelope> {
    let mut envelopes = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (address, payload, metadata) = parse_poll_line(line);
        let envelope = if driver_name == "mqtt" {
            build_mqtt_envelope(security, capabilities, &address, &payload, metadata)
        } else {
            build_generic_envelope(
                driver_name,
                security,
                capabilities,
                &address,
                &payload,
                metadata,
            )
        };
        envelopes.push(envelope);
    }
    envelopes
}

pub fn poll_baseband(registry: &Registry) -> ChannelBatch {
    let mut all_envelopes = Vec::new();

    registry.for_each(|name, handle| match handle.vtable.poll() {
        Ok(Some(raw)) => {
            let envelopes =
                parse_frontend_envelopes(name, handle.security_label, &handle.capabilities, &raw);
            all_envelopes.extend(envelopes);
        }
        Ok(None) => {}
        Err(e) => {
            log::warn!("gateway: poll {} failed: {}", name, e);
        }
    });

    // Post each envelope as InboundSignal to the unified actor mailbox
    let gw_actor_id = crate::state::actor_id();
    if gw_actor_id > 0 && !all_envelopes.is_empty() {
        if harmonia_actor_protocol::client::is_available() {
            for env in &all_envelopes {
                let envelope_sexp = env.to_sexp();
                let _ = harmonia_actor_protocol::client::post(
                    gw_actor_id,
                    0,
                    &format!(
                        "(:inbound-signal :envelope \"{}\")",
                        harmonia_actor_protocol::sexp_escape(&envelope_sexp)
                    ),
                );
            }
            // Heartbeat with envelope count
            let _ =
                harmonia_actor_protocol::client::heartbeat(gw_actor_id, all_envelopes.len() as u64);
        }
    }

    ChannelBatch {
        envelopes: all_envelopes,
        poll_timestamp_ms: now_ms(),
    }
}

pub fn send_signal(
    registry: &Registry,
    frontend_name: &str,
    sub_channel: &str,
    payload: &str,
) -> Result<(), String> {
    registry.with_frontend(frontend_name, |handle| {
        handle.vtable.send(sub_channel, payload)
    })?
}
