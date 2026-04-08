use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{VaultSignResult, CONFIG_COMPONENT};

pub(super) fn sign_with_vault(
    wallet: &PathBuf,
    label: &str,
    message: &str,
) -> Result<VaultSignResult, Box<dyn std::error::Error>> {
    let output = Command::new(resolve_hrmw_bin())
        .args([
            "key",
            "vault-sign",
            "--wallet",
            wallet.to_string_lossy().as_ref(),
            "--label",
            label,
            "--message",
            message,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "hrmw key vault-sign failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    parse_vault_sign_output(&String::from_utf8_lossy(&output.stdout))
}

pub(super) fn resolve_hrmw_bin() -> String {
    std::env::var("HARMONIA_HRMW_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("HRMW_BIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "hrmw".to_string())
}

pub(super) fn parse_vault_sign_output(output: &str) -> Result<VaultSignResult, Box<dyn std::error::Error>> {
    let label = parse_output_field(output, "Vault label:")?;
    let index = parse_output_field(output, "Vault index:")?.parse::<u32>()?;
    let public_key = parse_output_field(output, "Vault public key:")?;
    let signature = parse_output_field(output, "Signature:")?;
    Ok(VaultSignResult {
        label,
        index,
        public_key,
        signature,
    })
}

pub(super) fn parse_output_field(output: &str, prefix: &str) -> Result<String, Box<dyn std::error::Error>> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Ok(rest.trim().to_string());
        }
    }
    Err(format!("missing field in hrmw output: {prefix}").into())
}

pub(super) fn resolve_wallet_db_path() -> PathBuf {
    crate::paths::wallet_db_path().unwrap_or_else(|_| {
        std::env::var("HARMONIA_VAULT_WALLET_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("master.db"))
    })
}

pub(super) fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(super) fn config_or(scope: &str, key: &str, default: &str) -> String {
    harmonia_config_store::get_config_or(CONFIG_COMPONENT, scope, key, default)
        .unwrap_or_else(|_| default.to_string())
}

pub(super) fn config_required(scope: &str, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = harmonia_config_store::get_config(CONFIG_COMPONENT, scope, key)
        .map_err(|e| format!("config-store read failed for {scope}/{key}: {e}"))?
        .unwrap_or_default();
    if value.trim().is_empty() {
        return Err(format!("missing required config {scope}/{key}").into());
    }
    Ok(value)
}

pub(super) fn config_bool(scope: &str, key: &str, default: bool) -> bool {
    let value = config_or(scope, key, if default { "1" } else { "0" });
    value.trim().eq_ignore_ascii_case("1") || value.trim().eq_ignore_ascii_case("true")
}

pub(super) fn config_u64(scope: &str, key: &str, default: u64) -> u64 {
    config_or(scope, key, &default.to_string())
        .parse::<u64>()
        .unwrap_or(default)
}

pub(super) fn set_config(scope: &str, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    harmonia_config_store::set_config(CONFIG_COMPONENT, scope, key, value)
        .map_err(|e| format!("config-store write failed for {scope}/{key}: {e}").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use harmonia_transport_auth::normalize_fingerprint;

    #[test]
    fn parse_vault_sign_output_works() {
        let out = "Vault label:     mqtt-client-alice\nVault index:     1\nVault public key: ABCDEF\nSignature:       1234\n";
        let parsed = parse_vault_sign_output(out).expect("parse");
        assert_eq!(parsed.label, "mqtt-client-alice");
        assert_eq!(parsed.index, 1);
        assert_eq!(parsed.public_key, "ABCDEF");
        assert_eq!(parsed.signature, "1234");
    }

    #[test]
    fn normalize_fingerprint_uppercases_and_strips_separators() {
        assert_eq!(normalize_fingerprint("ab:cd-ef"), "ABCDEF");
    }
}
