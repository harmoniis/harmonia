use crate::model::{
    AuditContext, CanonicalMobileEnvelope, ChannelBatch, ChannelBody, ChannelEnvelope, ChannelRef,
    ConversationRef, OriginContext, PeerRef, SecurityContext, SecurityLabel, SessionContext,
    TransportContext,
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

fn default_channel_class(driver_name: &str) -> Option<String> {
    match driver_name {
        "mqtt" => Some("mqtt-device".to_string()),
        "http2" => Some("http2-client".to_string()),
        "tailscale" => Some("tailscale-agent".to_string()),
        "tui" => Some("local-tui".to_string()),
        "telegram" => Some("telegram-bot".to_string()),
        "whatsapp" => Some("whatsapp-bridge".to_string()),
        "signal" => Some("signal-bridge".to_string()),
        "imessage" => Some("imessage-bridge".to_string()),
        "slack" => Some("slack-bot".to_string()),
        "discord" => Some("discord-bot".to_string()),
        "email" => Some("email-imap".to_string()),
        "mattermost" => Some("mattermost-bot".to_string()),
        "nostr" => Some("nostr-relay".to_string()),
        _ => None,
    }
}

fn default_node_role(driver_name: &str) -> Option<String> {
    match driver_name {
        "mqtt" => Some("mqtt-client".to_string()),
        "http2" => Some("remote-user".to_string()),
        "tailscale" => Some("remote-node".to_string()),
        "email" | "telegram" | "whatsapp" | "signal" | "imessage" | "slack" | "discord"
        | "mattermost" | "nostr" => Some("remote-user".to_string()),
        _ => None,
    }
}

fn build_origin_context(
    driver_name: &str,
    address: &str,
    peer: &PeerRef,
    metadata: Option<&str>,
) -> Option<OriginContext> {
    let node_id = metadata_string_value(metadata, "node-id").or_else(|| match driver_name {
        "mqtt" => peer.device_id.clone().or_else(|| Some(peer.id.clone())),
        "tailscale" if !address.is_empty() => Some(address.to_string()),
        _ => None,
    });
    let node_label = metadata_string_value(metadata, "node-label").or_else(|| match driver_name {
        "mqtt" => peer.device_id.clone().or_else(|| Some(peer.id.clone())),
        "tailscale" if !address.is_empty() => Some(address.to_string()),
        _ => None,
    });
    let node_role =
        metadata_string_value(metadata, "node-role").or_else(|| default_node_role(driver_name));
    let channel_class = metadata_string_value(metadata, "channel-class")
        .or_else(|| default_channel_class(driver_name));
    let node_key_id = metadata_string_value(metadata, "node-key-id");
    let transport_security = metadata_string_value(metadata, "transport-security");
    let remote = metadata_bool_value(metadata, "remote").unwrap_or(matches!(
        driver_name,
        "mqtt"
            | "tailscale"
            | "http2"
            | "telegram"
            | "whatsapp"
            | "signal"
            | "imessage"
            | "slack"
            | "discord"
            | "email"
            | "mattermost"
            | "nostr"
    ));

    let node_id = node_id.or_else(|| node_label.clone())?;
    Some(OriginContext {
        node_id,
        node_label,
        node_role,
        channel_class,
        node_key_id,
        transport_security,
        remote,
    })
}

fn build_session_context(metadata: Option<&str>) -> Option<SessionContext> {
    let id = metadata_string_value(metadata, "session-id")?;
    let label = metadata_string_value(metadata, "session-label");
    Some(SessionContext { id, label })
}

fn build_generic_envelope(
    driver_name: &str,
    security: SecurityLabel,
    capabilities: &[crate::model::Capability],
    address: &str,
    payload: &str,
    metadata: Option<String>,
) -> ChannelEnvelope {
    let metadata_ref = metadata.as_deref();
    let fingerprint_valid = metadata_bool_value(metadata_ref, "fingerprint-valid").unwrap_or(true);
    let origin_fp = metadata_string_value(metadata_ref, "origin-fp");
    let peer_id = metadata_string_value(metadata_ref, "device-id")
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
    peer.agent_fp = metadata_string_value(metadata_ref, "agent-fp");
    peer.device_id = metadata_string_value(metadata_ref, "device-id");
    peer.platform = metadata_string_value(metadata_ref, "platform");
    peer.device_model = metadata_string_value(metadata_ref, "device-model");
    peer.app_version = metadata_string_value(metadata_ref, "app-version");
    peer.a2ui_version = metadata_string_value(metadata_ref, "a2ui-version");

    let label = if fingerprint_valid {
        security
    } else {
        SecurityLabel::Untrusted
    };
    let channel = ChannelRef::new(driver_name, address);
    let origin = build_origin_context(driver_name, address, &peer, metadata_ref);
    let session = build_session_context(metadata_ref);
    let conversation_id = session
        .as_ref()
        .map(|ctx| ctx.id.clone())
        .unwrap_or_else(|| channel.label.clone());
    let body = generic_body(payload);
    let body_text = body.text.clone();
    ChannelEnvelope {
        id: next_envelope_id(),
        version: 1,
        kind: "external".to_string(),
        type_name: generic_type_name(payload).to_string(),
        conversation: ConversationRef::new(conversation_id),
        channel: channel.clone(),
        peer,
        origin,
        session,
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
    let metadata_ref = metadata.as_deref();
    match serde_json::from_str::<CanonicalMobileEnvelope>(payload) {
        Ok(envelope) => {
            let fingerprint_valid =
                metadata_bool_value(metadata_ref, "fingerprint-valid").unwrap_or(true);
            let mut peer = PeerRef::new(
                metadata_string_value(metadata_ref, "device-id").unwrap_or_else(|| {
                    if envelope.client_fp.is_empty() {
                        topic.to_string()
                    } else {
                        envelope.client_fp.clone()
                    }
                }),
            );
            peer.origin_fp = if envelope.client_fp.is_empty() {
                metadata_string_value(metadata_ref, "origin-fp")
            } else {
                Some(envelope.client_fp.clone())
            };
            peer.agent_fp = if envelope.agent_fp.is_empty() {
                metadata_string_value(metadata_ref, "agent-fp")
            } else {
                Some(envelope.agent_fp.clone())
            };
            peer.device_id = metadata_string_value(metadata_ref, "device-id");
            peer.platform = metadata_string_value(metadata_ref, "platform");
            peer.device_model = metadata_string_value(metadata_ref, "device-model");
            peer.app_version = metadata_string_value(metadata_ref, "app-version");
            peer.a2ui_version = metadata_string_value(metadata_ref, "a2ui-version");

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
            let origin = build_origin_context("mqtt", topic, &peer, metadata_ref);
            let session = build_session_context(metadata_ref);
            let conversation_id = session
                .as_ref()
                .map(|ctx| ctx.id.clone())
                .unwrap_or_else(|| channel.label.clone());
            ChannelEnvelope {
                id: next_envelope_id(),
                version: envelope.v,
                kind: envelope.kind,
                type_name: envelope.type_name,
                conversation: ConversationRef::new(conversation_id),
                channel: channel.clone(),
                peer,
                origin,
                session,
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

/// Poll all registered frontends for inbound signals.
///
/// FFI-based frontend polling has been removed -- frontends are now ractor
/// actors that push envelopes directly. This function processes any envelopes
/// that arrive through the registry (currently none via FFI), applies sender
/// policy, payment interception, and command dispatch.
pub fn poll_baseband(registry: &Registry) -> ChannelBatch {
    // No FFI frontends to poll -- actor-based frontends push envelopes via
    // the runtime IPC system. The batch will be empty unless envelopes are
    // injected through some other path.
    let all_envelopes: Vec<ChannelEnvelope> = Vec::new();

    // Apply sender policy: deny-by-default for messaging frontends
    let all_envelopes: Vec<ChannelEnvelope> = all_envelopes
        .into_iter()
        .filter(|env| crate::sender_policy::is_signal_allowed(env))
        .collect();

    let all_envelopes = crate::payment_auth::intercept_paid_actions(registry, all_envelopes);

    // Intercept gateway commands (/wallet, /identity, etc.) — handle in Rust,
    // send response back to the originating frontend, filter them out so the
    // orchestrator only receives agent-level prompts.
    let all_envelopes = crate::command_dispatch::intercept_commands(registry, all_envelopes);

    ChannelBatch {
        envelopes: all_envelopes,
        poll_timestamp_ms: now_ms(),
    }
}

/// Send a signal to a frontend.
///
/// FFI-based frontend sending has been removed -- frontends are now ractor
/// actors. This stub returns an error; callers should use the actor mailbox.
pub fn send_signal(
    registry: &Registry,
    frontend_name: &str,
    _sub_channel: &str,
    _payload: &str,
) -> Result<(), String> {
    if !registry.is_registered(frontend_name) {
        return Err(format!("frontend not registered: {frontend_name}"));
    }
    // FFI send removed; actor-based frontends receive messages via their
    // ractor mailbox in the runtime.
    log::debug!(
        "gateway: send_signal to '{}' is a no-op (actor dispatch expected)",
        frontend_name
    );
    Ok(())
}
