//! [`SignalFrontend`] — actor-owned Signal bridge frontend.
//!
//! Speaks to a separate signal-cli REST proxy (typically `http://127.0.0.1:8080`)
//! whose URL + bearer token live in config-store / vault. The pairing helpers
//! (`pair_init`, `pair_status`) are stateless and live in [`crate::pairing`].

use async_trait::async_trait;

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

use crate::rpc::{
    extract_events, extract_first_string, extract_first_u64, get_json, parse_destination,
    post_json, RequestFailure,
};

pub(crate) const COMPONENT: &str = "signal-frontend";
pub(crate) const SIGNAL_ACCOUNT_SYMBOLS: &[&str] = &["signal-account"];
pub(crate) const SIGNAL_RPC_URL_SYMBOLS: &[&str] = &["signal-rpc-url", "signal-bridge-url"];
pub(crate) const SIGNAL_AUTH_TOKEN_SYMBOLS: &[&str] =
    &["signal-auth-token", "signal-auth-token-v2"];

pub struct SignalFrontend {
    rpc_url: String,
    account: String,
    auth_token: String,
    last_timestamp_ms: u64,
}

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
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

fn read_config_string(config: &str, keys: &[&str], store_key: &str) -> Option<String> {
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

fn read_config_string_with_legacy_vault(
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

#[async_trait]
impl Frontend for SignalFrontend {
    type Config = String;

    fn name() -> &'static str {
        "signal"
    }

    async fn init(config: String) -> Result<Self, String> {
        if let Some(token) = extract_sexp_string(&config, ":auth-token")
            .or_else(|| extract_sexp_string(&config, "auth-token"))
        {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                harmonia_vault::set_secret_for_symbol("signal-auth-token", trimmed)?;
            }
        }
        let rpc_url = read_config_string_with_legacy_vault(
            &config,
            &[":rpc-url", "rpc-url"],
            "rpc-url",
            SIGNAL_RPC_URL_SYMBOLS,
        )?
        .unwrap_or_else(|| "http://127.0.0.1:8080".to_string())
        .trim_end_matches('/')
        .to_string();
        let account = read_config_string_with_legacy_vault(
            &config,
            &[":account", "account"],
            "account",
            SIGNAL_ACCOUNT_SYMBOLS,
        )?
        .unwrap_or_default();
        let auth_token = read_vault_secret(SIGNAL_AUTH_TOKEN_SYMBOLS)?.unwrap_or_default();

        if account.is_empty() {
            return Err("missing account: set signal-frontend/account in config-store".into());
        }

        Ok(Self {
            rpc_url,
            account,
            auth_token,
            last_timestamp_ms: 0,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let receive_paths = [
            format!("{}/v1/receive/{}?timeout=1", self.rpc_url, self.account),
            format!("{}/v2/receive/{}?timeout=1", self.rpc_url, self.account),
        ];
        let mut payload = None;
        for endpoint in &receive_paths {
            match get_json(endpoint, &self.auth_token) {
                Ok(v) => {
                    payload = Some(v);
                    break;
                }
                Err(RequestFailure::NotFound) => continue,
                Err(RequestFailure::Other(msg)) => return Err(msg),
            }
        }
        let payload = match payload {
            Some(v) => v,
            None => return Err("signal receive endpoint not found (tried /v1 and /v2)".into()),
        };

        let events = extract_events(payload);
        if events.is_empty() {
            return Ok(Vec::new());
        }

        let mut outbound: Vec<InboundMessage> = Vec::new();
        let mut max_ts = self.last_timestamp_ms;

        for event in &events {
            let timestamp = extract_first_u64(
                event,
                &[
                    &["envelope", "timestamp"],
                    &["envelope", "dataMessage", "timestamp"],
                    &["timestamp"],
                ],
            )
            .unwrap_or(0);
            if timestamp != 0 && timestamp <= self.last_timestamp_ms {
                continue;
            }

            let text = extract_first_string(
                event,
                &[
                    &["envelope", "dataMessage", "message"],
                    &["envelope", "message"],
                    &["message"],
                    &["content", "message"],
                ],
            )
            .unwrap_or_default();
            if text.trim().is_empty() {
                continue;
            }

            let sender = extract_first_string(
                event,
                &[
                    &["envelope", "sourceNumber"],
                    &["envelope", "source"],
                    &["source"],
                    &["sender"],
                ],
            )
            .unwrap_or_else(|| "unknown".to_string());

            if timestamp > max_ts {
                max_ts = timestamp;
            }
            let metadata = format!(
                "(:channel-class \"signal-bridge\" :node-id \"{}\" :remote t)",
                sender.replace('\\', "\\\\").replace('"', "\\\"")
            );
            outbound.push(InboundMessage {
                address: sender,
                text,
                metadata: Some(metadata),
            });
        }

        if max_ts > self.last_timestamp_ms {
            self.last_timestamp_ms = max_ts;
        }
        Ok(outbound)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let (kind, target) = parse_destination(channel);
        if target.trim().is_empty() {
            return Err("signal target channel is empty".into());
        }
        let payload = if kind == "group" {
            serde_json::json!({
                "account": self.account,
                "groupId": target,
                "message": text,
            })
        } else {
            serde_json::json!({
                "account": self.account,
                "message": text,
                "number": [target],
                "recipients": [target],
            })
        };
        let send_paths = [
            format!("{}/v2/send", self.rpc_url),
            format!("{}/v1/send", self.rpc_url),
            format!("{}/v1/send/{}", self.rpc_url, self.account),
        ];
        for endpoint in &send_paths {
            match post_json(endpoint, &self.auth_token, &payload) {
                Ok(()) => return Ok(()),
                Err(RequestFailure::NotFound) => continue,
                Err(RequestFailure::Other(msg)) => return Err(msg),
            }
        }
        Err("signal send endpoint not found (tried /v2/send and /v1/send)".into())
    }

    async fn shutdown(&mut self) {
        self.rpc_url.clear();
        self.account.clear();
        self.auth_token.clear();
        self.last_timestamp_ms = 0;
    }
}
