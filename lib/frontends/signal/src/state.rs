use std::sync::{OnceLock, RwLock};

pub(crate) const COMPONENT: &str = "signal-frontend";
pub(crate) const SIGNAL_ACCOUNT_SYMBOLS: &[&str] = &["signal-account"];
pub(crate) const SIGNAL_RPC_URL_SYMBOLS: &[&str] = &["signal-rpc-url", "signal-bridge-url"];
pub(crate) const SIGNAL_AUTH_TOKEN_SYMBOLS: &[&str] =
    &["signal-auth-token", "signal-auth-token-v2"];

pub struct SignalState {
    pub rpc_url: String,
    pub account: String,
    pub auth_token: String,
    pub last_timestamp_ms: u64,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<SignalState>> = OnceLock::new();

pub(crate) fn state() -> &'static RwLock<SignalState> {
    STATE.get_or_init(|| {
        RwLock::new(SignalState {
            rpc_url: String::new(),
            account: String::new(),
            auth_token: String::new(),
            last_timestamp_ms: 0,
            initialized: false,
        })
    })
}

pub(crate) fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(sexp, key)
}

pub(crate) fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env()?;
    for symbol in symbols {
        let maybe = harmonia_vault::get_secret_for_component(COMPONENT, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(value) = maybe {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }
    Ok(None)
}

pub(crate) fn read_config_string(config: &str, keys: &[&str], store_key: &str) -> Option<String> {
    for key in keys {
        if let Some(v) = extract_sexp_string(config, key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, store_key, trimmed);
                return Some(trimmed.to_string());
            }
        }
    }
    harmonia_config_store::get_own(COMPONENT, store_key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(crate) fn read_config_string_with_legacy_vault(
    config: &str,
    keys: &[&str],
    store_key: &str,
    legacy_symbols: &[&str],
) -> Result<Option<String>, String> {
    if let Some(value) = read_config_string(config, keys, store_key) {
        return Ok(Some(value));
    }

    if let Some(legacy) = read_vault_secret(legacy_symbols)? {
        let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, store_key, &legacy);
        return Ok(Some(legacy));
    }

    Ok(None)
}

pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.rpc_url.clear();
        s.account.clear();
        s.auth_token.clear();
        s.last_timestamp_ms = 0;
        s.initialized = false;
    }
}

/// Resolve Signal config from in-process state, falling back to config-store.
/// This allows pair_init/pair_status to work from the CLI process where the
/// Signal frontend .so was never init()'d by the gateway.
pub(crate) fn resolve_signal_config() -> (String, String, String) {
    // Try in-process state first (populated when loaded as gateway plugin)
    if let Ok(s) = state().read() {
        if !s.rpc_url.is_empty() {
            return (s.rpc_url.clone(), s.account.clone(), s.auth_token.clone());
        }
    }
    // Fall back to config-store (works from CLI/TUI process)
    let rpc_url = harmonia_config_store::get_own(COMPONENT, "rpc-url")
        .ok()
        .flatten()
        .unwrap_or_default();
    let account = harmonia_config_store::get_own(COMPONENT, "account")
        .ok()
        .flatten()
        .unwrap_or_default();
    let auth_token = harmonia_config_store::get_own(COMPONENT, "auth-token")
        .ok()
        .flatten()
        .or_else(|| {
            SIGNAL_AUTH_TOKEN_SYMBOLS.iter().find_map(|sym| {
                harmonia_vault::get_secret_for_component(COMPONENT, sym)
                    .ok()
                    .flatten()
            })
        })
        .unwrap_or_default();
    (rpc_url, account, auth_token)
}
