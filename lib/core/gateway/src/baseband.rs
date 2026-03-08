use crate::model::{BasebandBatch, ChannelId, Signal, SignalDirection};
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

static SIGNAL_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_signal_id() -> u64 {
    SIGNAL_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Parse raw poll output from a frontend into Signal structs.
///
/// Frontends return newline-separated lines in one of these formats:
/// - 2-field: `sub_channel\tpayload` (backward compatible)
/// - 3-field: `sub_channel\tpayload\tmetadata` (with per-message metadata sexp)
/// - 1-field: `payload` (no sub-channel, no metadata)
///
/// The `capabilities_sexp` is the frontend's declared capabilities from baseband
/// config — attached to every signal from that frontend regardless of per-message metadata.
fn parse_frontend_signals(
    frontend_name: &str,
    security: crate::model::SecurityLabel,
    capabilities_sexp: Option<&str>,
    raw: &str,
) -> Vec<Signal> {
    let mut signals = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '\t').collect();
        let (sub_channel, payload, metadata) = match parts.len() {
            3 => (
                parts[0].to_string(),
                parts[1].to_string(),
                Some(parts[2].to_string()),
            ),
            2 => (parts[0].to_string(), parts[1].to_string(), None),
            _ => (String::new(), parts[0].to_string(), None),
        };
        let dissonance = compute_dissonance(&payload);
        signals.push(Signal {
            id: next_signal_id(),
            channel: ChannelId::new(frontend_name, sub_channel),
            security,
            payload,
            timestamp_ms: now_ms(),
            direction: SignalDirection::Inbound,
            metadata,
            capabilities: capabilities_sexp.map(|s| s.to_string()),
            dissonance,
        });
    }
    signals
}

pub fn poll_baseband(registry: &Registry) -> BasebandBatch {
    let mut all_signals = Vec::new();

    registry.for_each(|name, handle| match handle.vtable.poll() {
        Ok(Some(raw)) => {
            let caps_sexp = handle.capabilities_sexp();
            let caps = if caps_sexp == "nil" {
                None
            } else {
                Some(caps_sexp)
            };
            let signals =
                parse_frontend_signals(name, handle.security_label, caps.as_deref(), &raw);
            all_signals.extend(signals);
        }
        Ok(None) => {}
        Err(e) => {
            log::warn!("gateway: poll {} failed: {}", name, e);
        }
    });

    BasebandBatch {
        signals: all_signals,
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
