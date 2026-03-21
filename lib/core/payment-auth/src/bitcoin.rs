/// Bitcoin/ARK-specific payment logic.
use harmonia_config_store::{get_config, get_own, init as init_config_store};
use harmoniis_wallet::{
    ark::{ArkPaymentWallet, SqliteArkDb},
    bitcoin::DeterministicBitcoinWallet,
    RgbWallet,
};
use serde_json::json;
use std::path::Path;

use crate::challenge::open_or_create_wallet;
use crate::challenge::wallet_db_path;
use crate::model::{PaymentRequirement, COMPONENT};
use crate::policy::payment_header_for_rail;

pub(crate) fn rail_details_json(requirement: &PaymentRequirement) -> serde_json::Value {
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

pub(crate) fn bitcoin_receive_address() -> Result<String, String> {
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

pub(crate) fn configured_ark_asp_url() -> String {
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

pub(crate) fn bitcoin_db_path(master_wallet_path: &Path) -> std::path::PathBuf {
    crate::challenge::default_sidecar_path(master_wallet_path, "bitcoin.db")
}

pub(crate) fn bitcoin_network() -> bdk_wallet::bitcoin::Network {
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

pub(crate) fn settle_bitcoin(
    wallet_path: &Path,
    wallet: &RgbWallet,
    requirement: &PaymentRequirement,
    proof: &str,
    challenge_id: Option<&str>,
) -> Result<crate::model::SettlementReceipt, String> {
    use harmoniis_wallet::ark::parse_ark_proof;

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
    Ok(crate::model::SettlementReceipt {
        rail: "bitcoin".to_string(),
        settled_amount: verified.amount_sats.to_string(),
        payment_unit: requirement.unit.clone(),
        proof_ref: verified.txid,
        proof_kind: "ark_vtxo_txid".to_string(),
        challenge_id: challenge_id.map(ToString::to_string),
        txn_id: String::new(),
    })
}

// ── Tokio helpers ────────────────────────────────────────────────────

pub(crate) fn tokio_block_on<F>(future: F) -> Result<F::Output, String>
where
    F: std::future::Future,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("tokio runtime build failed: {e}"))?;
    Ok(runtime.block_on(future))
}

pub(crate) fn tokio_result<F, T, E>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio_block_on(future)?.map_err(|e| e.to_string())
}
