/// Challenge creation and recording.

use harmonia_baseband_channel_protocol::ChannelEnvelope;
use harmoniis_wallet::{NewPaymentTransaction, NewPaymentTransactionEvent};
use serde_json::json;

use crate::bitcoin::rail_details_json;
use crate::model::PaymentRequirement;
use crate::policy::payment_header_for_rail;
use crate::wallet::{open_or_create_wallet, wallet_db_path};

// Re-export settlement functions so existing `use challenge::*` callers work.
pub use crate::settlement::{append_settlement_metadata, settle_payment};

pub fn build_challenge_payload(
    requirement: &PaymentRequirement,
    code: &str,
    message: &str,
) -> String {
    let challenge_id = requirement
        .challenge_id
        .clone()
        .unwrap_or_else(|| challenge_id_for_action(&requirement.action));
    let preferred_rail = requirement
        .allowed_rails
        .first()
        .cloned()
        .unwrap_or_else(|| "webcash".to_string());
    let header = payment_header_for_rail(&preferred_rail);
    json!({
        "type": "payment_required",
        "status": 402,
        "code": code,
        "message": message,
        "challenge_id": challenge_id,
        "payment": {
            "action": requirement.action,
            "price": requirement.price,
            "payment_unit": requirement.unit,
            "allowed_rails": requirement.allowed_rails,
            "challenge_id": challenge_id,
            "expected_payment_rail": preferred_rail,
            "currency": preferred_rail,
            "header": header,
            "policy_id": requirement.policy_id,
            "note": requirement.note,
            "rail_details": rail_details_json(requirement),
        }
    })
    .to_string()
}

pub fn build_denied_payload(code: &str, message: &str) -> String {
    json!({
        "type": "payment_denied",
        "status": 403,
        "code": code,
        "message": message,
    })
    .to_string()
}

pub fn record_challenge(
    envelope: &ChannelEnvelope,
    requirement: &PaymentRequirement,
    challenge_id: &str,
) -> Result<String, String> {
    let wallet = open_or_create_wallet(&wallet_db_path()?)?;
    let metadata = json!({
        "frontend": envelope.channel.kind,
        "channel": envelope.channel.address,
        "origin_fp": envelope.peer.origin_fp,
        "session_id": envelope.session.as_ref().map(|value| value.id.as_str()),
    })
    .to_string();
    let txn_id = wallet
        .record_payment_transaction(&NewPaymentTransaction {
            attempt_id: None,
            occurred_at: None,
            direction: "inbound",
            role: "payee",
            source_system: "harmonia",
            service_origin: Some(&envelope.channel.kind),
            frontend_kind: Some(&envelope.channel.kind),
            transport_kind: Some(&envelope.transport.kind),
            endpoint_path: Some(&envelope.channel.address),
            method: None,
            session_id: envelope.session.as_ref().map(|value| value.id.as_str()),
            action_kind: &requirement.action,
            resource_ref: None,
            contract_ref: None,
            invoice_ref: None,
            challenge_id: Some(challenge_id),
            rail: requirement
                .allowed_rails
                .first()
                .map(|value| value.as_str())
                .unwrap_or("webcash"),
            payment_unit: &requirement.unit,
            quoted_amount: Some(&requirement.price),
            settled_amount: None,
            fee_amount: None,
            proof_ref: None,
            proof_kind: None,
            payer_ref: envelope.peer.origin_fp.as_deref(),
            payee_ref: Some("harmonia-agent"),
            request_hash: None,
            response_code: Some("payment_required"),
            status: "challenge_issued",
            metadata_json: Some(&metadata),
        })
        .map_err(|e| format!("record challenge transaction failed: {e}"))?;
    wallet
        .append_payment_transaction_event(&NewPaymentTransactionEvent {
            txn_id: &txn_id,
            event_type: "challenge_issued",
            status: "challenge_issued",
            actor: "gateway",
            details_json: Some(&metadata),
        })
        .map_err(|e| format!("record challenge event failed: {e}"))?;
    Ok(txn_id)
}

pub(crate) fn challenge_id_for_action(action: &str) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    format!("challenge-{}-{}", action.replace(' ', "-"), now)
}
