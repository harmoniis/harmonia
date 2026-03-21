/// Public API functions — envelope metadata extraction.
use harmonia_baseband_channel_protocol::ChannelEnvelope;

use crate::model::InboundPaymentMetadata;
use crate::policy::metadata_string_value;

pub fn extract_payment_metadata(envelope: &ChannelEnvelope) -> InboundPaymentMetadata {
    let mut payment = InboundPaymentMetadata {
        rail: metadata_string_value(envelope.transport.raw_metadata.as_deref(), "payment-rail"),
        proof: metadata_string_value(envelope.transport.raw_metadata.as_deref(), "payment-proof"),
        action_hint: metadata_string_value(
            envelope.transport.raw_metadata.as_deref(),
            "payment-action",
        ),
        challenge_id: metadata_string_value(
            envelope.transport.raw_metadata.as_deref(),
            "payment-challenge",
        ),
    };
    if let Some(value) = parse_payment_body(&envelope.body.raw) {
        payment.rail = payment
            .rail
            .or_else(|| json_string(&value, &["payment", "rail"]));
        payment.proof = payment
            .proof
            .or_else(|| json_string(&value, &["payment", "proof"]))
            .or_else(|| json_string(&value, &["payment", "secret"]))
            .or_else(|| json_string(&value, &["payment_proof"]));
        payment.action_hint = payment
            .action_hint
            .or_else(|| json_string(&value, &["payment", "action"]))
            .or_else(|| json_string(&value, &["payment_action"]));
        payment.challenge_id = payment
            .challenge_id
            .or_else(|| json_string(&value, &["payment", "challenge"]))
            .or_else(|| json_string(&value, &["payment", "challenge_id"]));
    }
    payment
}

fn parse_payment_body(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

fn json_string(value: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for part in path {
        current = current.get(*part)?;
    }
    current
        .as_str()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
