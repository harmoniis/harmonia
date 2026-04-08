/// Settlement -- verify and accept payment proofs per rail.

use harmonia_baseband_channel_protocol::ChannelEnvelope;
use harmoniis_wallet::{
    NewPaymentTransaction, NewPaymentTransactionEvent, RgbWallet, VoucherSecret,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use webylib::{Amount as WebcashAmount, SecretWebcash};

use crate::bitcoin::{settle_bitcoin, tokio_result};
use crate::model::{InboundPaymentMetadata, PaymentRequirement, SettlementReceipt};
use crate::policy::{escape_sexp, merge_metadata};
use crate::wallet::{open_or_create_wallet, open_voucher_wallet, open_webcash_wallet, wallet_db_path};

pub fn settle_payment(
    envelope: &ChannelEnvelope,
    requirement: &PaymentRequirement,
    payment: &InboundPaymentMetadata,
) -> Result<SettlementReceipt, String> {
    let rail = payment
        .rail
        .as_deref()
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| "payment rail missing".to_string())?;
    if !requirement
        .allowed_rails
        .iter()
        .any(|value| value.eq_ignore_ascii_case(&rail))
    {
        return Err(format!("payment rail '{rail}' is not allowed"));
    }
    let proof = payment
        .proof
        .as_deref()
        .ok_or_else(|| "payment proof missing".to_string())?;
    let wallet_path = wallet_db_path()?;
    let wallet = open_or_create_wallet(&wallet_path)?;
    let mut receipt = match rail.as_str() {
        "webcash" => settle_webcash(
            &wallet_path,
            &wallet,
            requirement,
            proof,
            payment.challenge_id.as_deref(),
        )?,
        "voucher" => settle_voucher(
            &wallet_path,
            &wallet,
            requirement,
            proof,
            payment.challenge_id.as_deref(),
        )?,
        "bitcoin" => settle_bitcoin(
            &wallet_path,
            &wallet,
            requirement,
            proof,
            payment.challenge_id.as_deref(),
        )?,
        _ => return Err(format!("unsupported payment rail '{rail}'")),
    };
    receipt.txn_id = record_settlement(&wallet, envelope, requirement, &receipt)?;
    Ok(receipt)
}

pub fn append_settlement_metadata(
    base_metadata: Option<&str>,
    receipt: &SettlementReceipt,
) -> String {
    let extra = format!(
        "(:payment-authenticated t :payment-rail \"{}\" :payment-proof-ref \"{}\" :payment-proof-kind \"{}\" :payment-amount \"{}\" :payment-unit \"{}\")",
        escape_sexp(&receipt.rail),
        escape_sexp(&receipt.proof_ref),
        escape_sexp(&receipt.proof_kind),
        escape_sexp(&receipt.settled_amount),
        escape_sexp(&receipt.payment_unit),
    );
    merge_metadata(base_metadata, &extra)
}

// ── Per-rail settlement ──────────────────────────────────────────────

fn settle_webcash(
    wallet_path: &Path,
    wallet: &RgbWallet,
    requirement: &PaymentRequirement,
    proof: &str,
    challenge_id: Option<&str>,
) -> Result<SettlementReceipt, String> {
    let secret = SecretWebcash::parse(proof).map_err(|e| format!("invalid webcash proof: {e}"))?;
    let required_wats = requirement
        .price
        .parse::<u64>()
        .map_err(|_| format!("invalid webcash price '{}'", requirement.price))?;
    let required_amount = WebcashAmount::from_wats(required_wats as i64);
    if secret.amount != required_amount {
        return Err(format!(
            "webcash amount mismatch: received {}, expected {}",
            secret.amount, required_amount
        ));
    }
    let webcash_wallet = open_webcash_wallet(wallet_path, wallet)?;
    tokio_result(webcash_wallet.insert(secret))
        .map_err(|e| format!("failed to insert webcash into wallet: {e}"))?;
    Ok(SettlementReceipt {
        rail: "webcash".to_string(),
        settled_amount: requirement.price.clone(),
        payment_unit: requirement.unit.clone(),
        proof_ref: hash_reference(proof),
        proof_kind: "webcash_secret_hash".to_string(),
        challenge_id: challenge_id.map(ToString::to_string),
        txn_id: String::new(),
    })
}

fn settle_voucher(
    wallet_path: &Path,
    wallet: &RgbWallet,
    requirement: &PaymentRequirement,
    proof: &str,
    challenge_id: Option<&str>,
) -> Result<SettlementReceipt, String> {
    let secret = VoucherSecret::parse(proof).map_err(|e| format!("invalid voucher proof: {e}"))?;
    let required_units = requirement
        .price
        .parse::<u64>()
        .map_err(|_| format!("invalid voucher price '{}'", requirement.price))?;
    if secret.amount_units != required_units {
        return Err(format!(
            "voucher amount mismatch: received {}, expected {}",
            secret.amount_units, required_units
        ));
    }
    let voucher_wallet = open_voucher_wallet(wallet_path, wallet)?;
    voucher_wallet
        .insert(secret.clone())
        .map_err(|e| format!("failed to insert voucher into wallet: {e}"))?;
    Ok(SettlementReceipt {
        rail: "voucher".to_string(),
        settled_amount: requirement.price.clone(),
        payment_unit: requirement.unit.clone(),
        proof_ref: secret.public_proof().public_hash,
        proof_kind: "voucher_public_hash".to_string(),
        challenge_id: challenge_id.map(ToString::to_string),
        txn_id: String::new(),
    })
}

fn record_settlement(
    wallet: &RgbWallet,
    envelope: &ChannelEnvelope,
    requirement: &PaymentRequirement,
    receipt: &SettlementReceipt,
) -> Result<String, String> {
    let metadata = json!({
        "frontend": envelope.channel.kind,
        "channel": envelope.channel.address,
        "origin_fp": envelope.peer.origin_fp,
        "session_id": envelope.session.as_ref().map(|value| value.id.as_str()),
        "proof_kind": receipt.proof_kind,
    })
    .to_string();
    let payer_ref = envelope
        .peer
        .origin_fp
        .as_deref()
        .or(Some(envelope.peer.id.as_str()));
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
            challenge_id: receipt.challenge_id.as_deref(),
            rail: &receipt.rail,
            payment_unit: &receipt.payment_unit,
            quoted_amount: Some(&requirement.price),
            settled_amount: Some(&receipt.settled_amount),
            fee_amount: None,
            proof_ref: Some(&receipt.proof_ref),
            proof_kind: Some(&receipt.proof_kind),
            payer_ref,
            payee_ref: Some("harmonia-agent"),
            request_hash: None,
            response_code: Some("accepted"),
            status: "succeeded",
            metadata_json: Some(&metadata),
        })
        .map_err(|e| format!("record payment transaction failed: {e}"))?;
    wallet
        .append_payment_transaction_event(&NewPaymentTransactionEvent {
            txn_id: &txn_id,
            event_type: "settled",
            status: "succeeded",
            actor: "payment-auth",
            details_json: Some(&metadata),
        })
        .map_err(|e| format!("record payment event failed: {e}"))?;
    Ok(txn_id)
}

pub(crate) fn hash_reference(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}
