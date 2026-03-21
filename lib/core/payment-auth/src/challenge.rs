/// Challenge creation, verification, and settlement.
use harmonia_baseband_channel_protocol::ChannelEnvelope;
use harmonia_config_store::{get_config, init as init_config_store};
use harmoniis_wallet::{
    NewPaymentTransaction, NewPaymentTransactionEvent, RgbWallet, VoucherSecret, VoucherWallet,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use webylib::{Amount as WebcashAmount, SecretWebcash, Wallet as WebcashWallet};

use crate::bitcoin::{rail_details_json, settle_bitcoin, tokio_result};
use crate::model::{InboundPaymentMetadata, PaymentRequirement, SettlementReceipt, COMPONENT};
use crate::policy::{escape_sexp, merge_metadata, payment_header_for_rail};

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

// ── Settlement helpers (per rail) ────────────────────────────────────

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

// ── Wallet helpers ───────────────────────────────────────────────────

pub(crate) fn wallet_db_path() -> Result<PathBuf, String> {
    let _ = init_config_store();
    if let Ok(Some(root)) = get_config(COMPONENT, "global", "wallet-root") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("master.db"));
        }
    }
    if let Ok(Some(path)) = get_config(COMPONENT, "global", "wallet-db") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(root) = std::env::var("HARMONIA_WALLET_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed).join("master.db"));
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    Ok(PathBuf::from(home)
        .join(".harmoniis")
        .join("wallet")
        .join("master.db"))
}

pub(crate) fn open_or_create_wallet(path: &Path) -> Result<RgbWallet, String> {
    if path.exists() {
        RgbWallet::open(path).map_err(|e| format!("open wallet failed: {e}"))
    } else {
        RgbWallet::create(path).map_err(|e| format!("create wallet failed: {e}"))
    }
}

fn open_webcash_wallet(
    master_wallet_path: &Path,
    wallet: &RgbWallet,
) -> Result<WebcashWallet, String> {
    let path = default_sidecar_path(master_wallet_path, "webcash.db");
    let webcash_wallet = tokio_result(WebcashWallet::open(&path))
        .map_err(|e| format!("open webcash wallet failed: {e}"))?;
    let master_secret = wallet
        .derive_webcash_master_secret_hex()
        .map_err(|e| format!("derive webcash master secret failed: {e}"))?;
    tokio_result(webcash_wallet.store_master_secret(&master_secret))
        .map_err(|e| format!("store webcash master secret failed: {e}"))?;
    Ok(webcash_wallet)
}

fn open_voucher_wallet(
    master_wallet_path: &Path,
    wallet: &RgbWallet,
) -> Result<VoucherWallet, String> {
    let path = default_sidecar_path(master_wallet_path, "voucher.db");
    let voucher_wallet =
        VoucherWallet::open(&path).map_err(|e| format!("open voucher wallet failed: {e}"))?;
    let master_secret = wallet
        .derive_voucher_master_secret_hex()
        .map_err(|e| format!("derive voucher master secret failed: {e}"))?;
    voucher_wallet
        .store_master_secret(&master_secret)
        .map_err(|e| format!("store voucher master secret failed: {e}"))?;
    Ok(voucher_wallet)
}

pub(crate) fn default_sidecar_path(master_wallet_path: &Path, file_name: &str) -> PathBuf {
    master_wallet_path
        .parent()
        .map(|value| value.join(file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

pub(crate) fn challenge_id_for_action(action: &str) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    format!("challenge-{}-{}", action.replace(' ', "-"), now)
}

pub(crate) fn hash_reference(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}
