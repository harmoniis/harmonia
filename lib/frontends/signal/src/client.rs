use crate::state::{
    extract_sexp_string, read_config_string_with_legacy_vault, read_vault_secret, state,
    SIGNAL_ACCOUNT_SYMBOLS, SIGNAL_AUTH_TOKEN_SYMBOLS, SIGNAL_RPC_URL_SYMBOLS,
};

pub use crate::messaging::{poll, send};
pub use crate::pairing::{pair_init, pair_status};
pub use crate::state::shutdown;

pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;
    if s.initialized {
        return Err("signal already initialized".into());
    }

    if let Some(token) = extract_sexp_string(config, ":auth-token")
        .or_else(|| extract_sexp_string(config, "auth-token"))
    {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("signal-auth-token", trimmed)?;
        }
    }

    s.rpc_url = read_config_string_with_legacy_vault(
        config,
        &[":rpc-url", "rpc-url"],
        "rpc-url",
        SIGNAL_RPC_URL_SYMBOLS,
    )?
    .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
    .trim_end_matches('/')
    .to_string();
    s.account = read_config_string_with_legacy_vault(
        config,
        &[":account", "account"],
        "account",
        SIGNAL_ACCOUNT_SYMBOLS,
    )?
    .unwrap_or_default();
    s.auth_token = read_vault_secret(SIGNAL_AUTH_TOKEN_SYMBOLS)?.unwrap_or_default();

    if s.account.is_empty() {
        return Err("missing account: set signal-frontend/account in config-store".into());
    }

    s.last_timestamp_ms = 0;
    s.initialized = true;
    Ok(())
}
