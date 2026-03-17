use harmonia_baseband_channel_protocol::ChannelEnvelope;
use harmonia_config_store::{get_config, get_own, init as init_config_store};
use harmoniis_wallet::{
    ark::{parse_ark_proof, ArkPaymentWallet, SqliteArkDb},
    bitcoin::DeterministicBitcoinWallet,
    NewPaymentTransaction, NewPaymentTransactionEvent, RgbWallet, VoucherSecret, VoucherWallet,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use webylib::{Amount as WebcashAmount, SecretWebcash, Wallet as WebcashWallet};

const COMPONENT: &str = "payment-auth";
const DEFAULT_ALLOWED_RAILS: &[&str] = &["webcash", "voucher", "bitcoin"];

#[derive(Debug, Clone, Default)]
pub struct InboundPaymentMetadata {
    pub rail: Option<String>,
    pub proof: Option<String>,
    pub action_hint: Option<String>,
    pub challenge_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum PolicyDecision {
    Free,
    Deny { code: String, message: String },
    Pay(PaymentRequirement),
}

#[derive(Debug, Clone)]
pub struct PaymentRequirement {
    pub action: String,
    pub price: String,
    pub unit: String,
    pub allowed_rails: Vec<String>,
    pub challenge_id: Option<String>,
    pub policy_id: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SettlementReceipt {
    pub rail: String,
    pub settled_amount: String,
    pub payment_unit: String,
    pub proof_ref: String,
    pub proof_kind: String,
    pub challenge_id: Option<String>,
    pub txn_id: String,
}

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

pub fn build_policy_query(envelope: &ChannelEnvelope, payment: &InboundPaymentMetadata) -> String {
    let session_id = envelope
        .session
        .as_ref()
        .map(|value| value.id.as_str())
        .unwrap_or("");
    let origin_fp = envelope.peer.origin_fp.as_deref().unwrap_or("");
    let requested_action = payment
        .action_hint
        .as_deref()
        .unwrap_or_else(|| default_action_hint(envelope));
    format!(
        "(:frontend \"{}\" :address \"{}\" :session-id \"{}\" :origin-fp \"{}\" :requested-action \"{}\" :type-name \"{}\" :body-format \"{}\" :body-text \"{}\" :security-label \"{}\")",
        escape_sexp(&envelope.channel.kind),
        escape_sexp(&envelope.channel.address),
        escape_sexp(session_id),
        escape_sexp(origin_fp),
        escape_sexp(requested_action),
        escape_sexp(&envelope.type_name),
        escape_sexp(&envelope.body.format),
        escape_sexp(&trim_for_policy(&envelope.body.text)),
        escape_sexp(envelope.security.label.as_str()),
    )
}

pub fn parse_policy_response(raw: &str) -> Result<PolicyDecision, String> {
    let mode = sexp_symbol_value(raw, "mode").unwrap_or_else(|| "free".to_string());
    match mode.as_str() {
        "free" => Ok(PolicyDecision::Free),
        "deny" => Ok(PolicyDecision::Deny {
            code: sexp_string_value(raw, "code").unwrap_or_else(|| "payment_denied".to_string()),
            message: sexp_string_value(raw, "message")
                .unwrap_or_else(|| "Payment policy denied this action.".to_string()),
        }),
        "pay" => {
            let action = sexp_string_value(raw, "action")
                .ok_or_else(|| "payment policy response missing :action".to_string())?;
            let price = sexp_string_value(raw, "price")
                .ok_or_else(|| "payment policy response missing :price".to_string())?;
            let unit = sexp_string_value(raw, "unit")
                .ok_or_else(|| "payment policy response missing :unit".to_string())?;
            let allowed_rails = sexp_string_list(raw, "allowed-rails")
                .into_iter()
                .map(|value| value.to_ascii_lowercase())
                .filter(|value| is_supported_rail(value))
                .collect::<Vec<_>>();
            if allowed_rails.is_empty() {
                return Err("payment policy response missing supported rails".to_string());
            }
            Ok(PolicyDecision::Pay(PaymentRequirement {
                action,
                price,
                unit,
                allowed_rails,
                challenge_id: sexp_string_value(raw, "challenge-id"),
                policy_id: sexp_string_value(raw, "policy-id"),
                note: sexp_string_value(raw, "note"),
            }))
        }
        other => Err(format!("unknown payment policy mode '{other}'")),
    }
}

pub fn default_policy_response(requested_action: &str) -> PolicyDecision {
    let action = requested_action.trim().to_ascii_lowercase();
    if action.is_empty() {
        return PolicyDecision::Free;
    }
    let _ = init_config_store();
    let price_key = format!("{action}-price");
    let mode_key = format!("{action}-mode");
    let unit_key = format!("{action}-unit");
    let rails_key = format!("{action}-allowed-rails");
    let mode = get_own(COMPONENT, &mode_key)
        .ok()
        .flatten()
        .unwrap_or_else(|| "free".to_string())
        .trim()
        .to_ascii_lowercase();
    if mode == "deny" {
        return PolicyDecision::Deny {
            code: "payment_denied".to_string(),
            message: format!("Payment policy denied action '{action}'."),
        };
    }
    let Some(price) = get_own(COMPONENT, &price_key).ok().flatten() else {
        return PolicyDecision::Free;
    };
    let unit = get_own(COMPONENT, &unit_key)
        .ok()
        .flatten()
        .unwrap_or_else(|| default_unit_for_action(&action));
    let allowed_rails = get_own(COMPONENT, &rails_key)
        .ok()
        .flatten()
        .map(|value| parse_csv_rails(&value))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            DEFAULT_ALLOWED_RAILS
                .iter()
                .map(|value| (*value).to_string())
                .collect()
        });
    PolicyDecision::Pay(PaymentRequirement {
        action,
        price,
        unit,
        allowed_rails,
        challenge_id: None,
        policy_id: Some("config-store".to_string()),
        note: None,
    })
}

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

fn settle_bitcoin(
    wallet_path: &Path,
    wallet: &RgbWallet,
    requirement: &PaymentRequirement,
    proof: &str,
    challenge_id: Option<&str>,
) -> Result<SettlementReceipt, String> {
    let (txid, proof_amount) =
        parse_ark_proof(proof).ok_or_else(|| "invalid bitcoin proof format".to_string())?;
    let required_sats = requirement
        .price
        .parse::<u64>()
        .map_err(|_| format!("invalid bitcoin price '{}'", requirement.price))?;
    if proof_amount != required_sats {
        return Err(format!(
            "bitcoin amount mismatch: received {proof_amount}, expected {required_sats}"
        ));
    }
    let asp_url = configured_ark_asp_url();
    let btc_wallet = DeterministicBitcoinWallet::from_master_wallet(wallet, bitcoin_network())
        .map_err(|e| format!("failed to derive bitcoin wallet: {e}"))?;
    let db = SqliteArkDb::open(&bitcoin_db_path(wallet_path))
        .map_err(|e| format!("failed to open bitcoin wallet db: {e}"))?;
    let verified = tokio_result(async {
        let ark = ArkPaymentWallet::connect(&btc_wallet, &asp_url, db).await?;
        ark.verify_incoming_vtxo(&txid, required_sats).await
    })
    .map_err(|e| format!("failed to verify bitcoin proof: {e}"))?;
    Ok(SettlementReceipt {
        rail: "bitcoin".to_string(),
        settled_amount: verified.amount_sats.to_string(),
        payment_unit: requirement.unit.clone(),
        proof_ref: verified.txid,
        proof_kind: "ark_vtxo_txid".to_string(),
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

fn rail_details_json(requirement: &PaymentRequirement) -> serde_json::Value {
    let mut rails = serde_json::Map::new();
    for rail in &requirement.allowed_rails {
        match rail.as_str() {
            "webcash" => {
                rails.insert(
                    "webcash".to_string(),
                    json!({ "header": payment_header_for_rail("webcash") }),
                );
            }
            "voucher" => {
                rails.insert(
                    "voucher".to_string(),
                    json!({ "header": payment_header_for_rail("voucher") }),
                );
            }
            "bitcoin" => {
                if let Ok(address) = bitcoin_receive_address() {
                    rails.insert(
                        "bitcoin".to_string(),
                        json!({
                            "mode": "ark",
                            "asp_url": configured_ark_asp_url(),
                            "offchain_receive_address": address,
                            "header": payment_header_for_rail("bitcoin"),
                        }),
                    );
                }
            }
            _ => {}
        }
    }
    serde_json::Value::Object(rails)
}

fn bitcoin_receive_address() -> Result<String, String> {
    let wallet_path = wallet_db_path()?;
    let wallet = open_or_create_wallet(&wallet_path)?;
    let asp_url = configured_ark_asp_url();
    let btc_wallet = DeterministicBitcoinWallet::from_master_wallet(&wallet, bitcoin_network())
        .map_err(|e| format!("failed to derive bitcoin wallet: {e}"))?;
    let db = SqliteArkDb::open(&bitcoin_db_path(&wallet_path))
        .map_err(|e| format!("failed to open bitcoin wallet db: {e}"))?;
    tokio_result(async {
        let ark = ArkPaymentWallet::connect(&btc_wallet, &asp_url, db).await?;
        ark.get_offchain_address()
    })
    .map_err(|e| format!("failed to derive bitcoin receive address: {e}"))
}

fn configured_ark_asp_url() -> String {
    let _ = init_config_store();
    get_own(COMPONENT, "bitcoin-asp-url")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            get_config(COMPONENT, "global", "bitcoin-asp-url")
                .ok()
                .flatten()
        })
        .or_else(|| std::env::var("HARMONIA_ARK_ASP_URL").ok())
        .unwrap_or_else(|| "https://arkade.computer".to_string())
}

fn wallet_db_path() -> Result<PathBuf, String> {
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

fn open_or_create_wallet(path: &Path) -> Result<RgbWallet, String> {
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

fn default_sidecar_path(master_wallet_path: &Path, file_name: &str) -> PathBuf {
    master_wallet_path
        .parent()
        .map(|value| value.join(file_name))
        .unwrap_or_else(|| PathBuf::from(file_name))
}

fn bitcoin_db_path(master_wallet_path: &Path) -> PathBuf {
    default_sidecar_path(master_wallet_path, "bitcoin.db")
}

fn bitcoin_network() -> bdk_wallet::bitcoin::Network {
    std::env::var("HARMONIA_BITCOIN_NETWORK")
        .ok()
        .as_deref()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "testnet" => bdk_wallet::bitcoin::Network::Testnet,
            "signet" => bdk_wallet::bitcoin::Network::Signet,
            "regtest" => bdk_wallet::bitcoin::Network::Regtest,
            _ => bdk_wallet::bitcoin::Network::Bitcoin,
        })
        .unwrap_or(bdk_wallet::bitcoin::Network::Bitcoin)
}

fn default_action_hint(envelope: &ChannelEnvelope) -> &str {
    if envelope.type_name.starts_with("payment.") {
        "paid-message"
    } else {
        "message"
    }
}

fn default_unit_for_action(action: &str) -> String {
    match action {
        "identity" | "post" | "comment" | "rate" => "wats".to_string(),
        _ => "wats".to_string(),
    }
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

fn metadata_string_value(metadata: Option<&str>, key: &str) -> Option<String> {
    let meta = metadata?;
    let needle = format!(":{} \"", key);
    if let Some(start) = find_case_insensitive(meta, &needle) {
        let from = start + needle.len();
        let rest = &meta[from..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    let needle = format!(":{} ", key);
    let start = find_case_insensitive(meta, &needle)?;
    let from = start + needle.len();
    let rest = &meta[from..];
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == ')')
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn sexp_string_value(raw: &str, key: &str) -> Option<String> {
    metadata_string_value(Some(raw), key)
}

fn sexp_symbol_value(raw: &str, key: &str) -> Option<String> {
    let needle = format!(":{} :", key);
    let start = find_case_insensitive(raw, &needle)?;
    let rest = &raw[start + needle.len()..];
    let end = rest
        .find(|ch: char| ch.is_whitespace() || ch == ')')
        .unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_ascii_lowercase())
    }
}

fn sexp_string_list(raw: &str, key: &str) -> Vec<String> {
    let needle = format!(":{} (", key);
    let Some(start) = find_case_insensitive(raw, &needle) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let rest = &raw[start + needle.len()..];
    let end = rest.find(')').unwrap_or(rest.len());
    let mut cursor = rest[..end].trim();
    while !cursor.is_empty() {
        if let Some(stripped) = cursor.strip_prefix('"') {
            let Some(end_quote) = stripped.find('"') else {
                break;
            };
            out.push(stripped[..end_quote].to_string());
            cursor = stripped[end_quote + 1..].trim_start();
        } else {
            let split = cursor.find(char::is_whitespace).unwrap_or(cursor.len());
            let value = cursor[..split].trim();
            if !value.is_empty() {
                out.push(value.trim_matches(':').to_string());
            }
            cursor = cursor[split..].trim_start();
        }
    }
    out
}

fn parse_csv_rails(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| is_supported_rail(value))
        .collect()
}

fn is_supported_rail(value: &str) -> bool {
    matches!(value, "webcash" | "voucher" | "bitcoin")
}

fn merge_metadata(base: Option<&str>, extra: &str) -> String {
    fn trim_parens(value: &str) -> &str {
        let trimmed = value.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    }
    match base.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => format!("({} {})", trim_parens(value), trim_parens(extra)),
        None => extra.to_string(),
    }
}

fn challenge_id_for_action(action: &str) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    format!("challenge-{}-{}", action.replace(' ', "-"), now)
}

fn trim_for_policy(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 256 {
        trimmed.to_string()
    } else {
        trimmed[..256].to_string()
    }
}

fn escape_sexp(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
}

fn payment_header_for_rail(rail: &str) -> &'static str {
    match rail {
        "voucher" => "X-Voucher-Secret",
        "bitcoin" => "X-Bitcoin-Secret",
        _ => "X-Webcash-Secret",
    }
}

fn hash_reference(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

fn tokio_block_on<F>(future: F) -> Result<F::Output, String>
where
    F: std::future::Future,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime build failed: {e}"))?;
    Ok(runtime.block_on(future))
}

fn tokio_result<F, T, E>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio_block_on(future)?.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_challenge_payload, extract_payment_metadata, parse_policy_response,
        PaymentRequirement, PolicyDecision,
    };
    use harmonia_baseband_channel_protocol::{
        AuditContext, ChannelBody, ChannelEnvelope, ChannelRef, ConversationRef, PeerRef,
        SecurityContext, SecurityLabel, TransportContext,
    };

    fn sample_envelope(body: &str, metadata: Option<&str>) -> ChannelEnvelope {
        ChannelEnvelope {
            id: 1,
            version: 1,
            kind: "external".to_string(),
            type_name: "message.structured".to_string(),
            conversation: ConversationRef::new("conv"),
            channel: ChannelRef::new("http2", "peer/session/default"),
            peer: PeerRef::new("peer"),
            origin: None,
            session: None,
            body: ChannelBody {
                format: "json".to_string(),
                text: body.to_string(),
                raw: body.to_string(),
            },
            capabilities: vec![],
            security: SecurityContext {
                label: SecurityLabel::Authenticated,
                source: "test".to_string(),
                fingerprint_valid: true,
            },
            audit: AuditContext {
                timestamp_ms: 0,
                dissonance: 0.0,
            },
            attachments: vec![],
            transport: TransportContext {
                kind: "http2".to_string(),
                raw_address: "peer/session/default".to_string(),
                raw_metadata: metadata.map(ToString::to_string),
            },
        }
    }

    #[test]
    fn extracts_payment_metadata_from_json_and_metadata() {
        let envelope = sample_envelope(
            r#"{"payment":{"proof":"secret-1","action":"post","challenge":"c-1"}}"#,
            Some("(:payment-rail \"voucher\")"),
        );
        let payment = extract_payment_metadata(&envelope);
        assert_eq!(payment.rail.as_deref(), Some("voucher"));
        assert_eq!(payment.proof.as_deref(), Some("secret-1"));
        assert_eq!(payment.action_hint.as_deref(), Some("post"));
        assert_eq!(payment.challenge_id.as_deref(), Some("c-1"));
    }

    #[test]
    fn parses_pay_policy_response() {
        let decision = parse_policy_response(
            "(:mode :pay :action \"post\" :price \"42\" :unit \"credits\" :allowed-rails (\"voucher\" \"bitcoin\") :policy-id \"cfg\")",
        )
        .expect("parse pay response");
        match decision {
            PolicyDecision::Pay(requirement) => {
                assert_eq!(requirement.action, "post");
                assert_eq!(requirement.price, "42");
                assert_eq!(requirement.unit, "credits");
                assert_eq!(requirement.allowed_rails, vec!["voucher", "bitcoin"]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn parses_uppercase_policy_symbols() {
        let decision = parse_policy_response(
            "(:MODE :PAY :ACTION \"post\" :PRICE \"42\" :UNIT \"credits\" :ALLOWED-RAILS (\"voucher\" \"bitcoin\") :POLICY-ID \"cfg\")",
        )
        .expect("parse uppercase pay response");
        match decision {
            PolicyDecision::Pay(requirement) => {
                assert_eq!(requirement.action, "post");
                assert_eq!(requirement.allowed_rails, vec!["voucher", "bitcoin"]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn challenge_payload_keeps_legacy_fields() {
        let payload = build_challenge_payload(
            &PaymentRequirement {
                action: "post".to_string(),
                price: "42".to_string(),
                unit: "credits".to_string(),
                allowed_rails: vec!["voucher".to_string(), "bitcoin".to_string()],
                challenge_id: Some("challenge-123".to_string()),
                policy_id: Some("cfg".to_string()),
                note: None,
            },
            "payment_required",
            "pay first",
        );
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
        assert_eq!(parsed["challenge_id"], "challenge-123");
        assert_eq!(parsed["payment"]["challenge_id"], "challenge-123");
        assert_eq!(parsed["payment"]["expected_payment_rail"], "voucher");
        assert_eq!(parsed["payment"]["currency"], "voucher");
        assert_eq!(parsed["payment"]["header"], "X-Voucher-Secret");
    }
}
