/// Wallet helpers -- path resolution, opening, and sidecar wallets.

use harmonia_config_store::{get_config, init as init_config_store};
use harmoniis_wallet::{RgbWallet, VoucherWallet};
use std::path::{Path, PathBuf};
use webylib::{Wallet as WebcashWallet};

use crate::bitcoin::tokio_result;
use crate::model::COMPONENT;

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

pub(crate) fn open_webcash_wallet(
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

pub(crate) fn open_voucher_wallet(
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
