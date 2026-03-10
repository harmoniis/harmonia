use serde_json::Value;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "signal-frontend";
const SIGNAL_ACCOUNT_SYMBOLS: &[&str] = &["signal-account"];
const SIGNAL_RPC_URL_SYMBOLS: &[&str] = &["signal-rpc-url", "signal-bridge-url"];
const SIGNAL_AUTH_TOKEN_SYMBOLS: &[&str] = &["signal-auth-token", "signal-auth-token-v2"];

pub struct SignalState {
    pub rpc_url: String,
    pub account: String,
    pub auth_token: String,
    pub last_timestamp_ms: u64,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<SignalState>> = OnceLock::new();

fn state() -> &'static RwLock<SignalState> {
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

fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pattern = format!("({key} \"");
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
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

enum RequestFailure {
    NotFound,
    Other(String),
}

fn apply_auth(req: ureq::Request, auth_token: &str) -> ureq::Request {
    if auth_token.is_empty() {
        req
    } else {
        req.set("Authorization", &format!("Bearer {auth_token}"))
    }
}

fn error_from_ureq(err: ureq::Error) -> RequestFailure {
    match err {
        ureq::Error::Status(code, resp) => {
            if code == 404 {
                return RequestFailure::NotFound;
            }
            let body = resp.into_string().unwrap_or_default();
            let msg = if body.is_empty() {
                format!("signal api status {code}")
            } else {
                format!("signal api status {code}: {body}")
            };
            RequestFailure::Other(msg)
        }
        ureq::Error::Transport(t) => RequestFailure::Other(format!("signal transport error: {t}")),
    }
}

fn get_json(url: &str, auth_token: &str) -> Result<Value, RequestFailure> {
    apply_auth(ureq::get(url), auth_token)
        .call()
        .map_err(error_from_ureq)?
        .into_json()
        .map_err(|e| RequestFailure::Other(format!("signal json decode failed: {e}")))
}

fn post_json(url: &str, auth_token: &str, body: &Value) -> Result<(), RequestFailure> {
    apply_auth(ureq::post(url), auth_token)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(error_from_ureq)?;
    Ok(())
}

fn get_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

fn extract_first_string(root: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(v) = get_path(root, path).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn extract_first_u64(root: &Value, paths: &[&[&str]]) -> Option<u64> {
    for path in paths {
        if let Some(v) = get_path(root, path).and_then(Value::as_u64) {
            return Some(v);
        }
    }
    None
}

fn extract_events(payload: Value) -> Vec<Value> {
    if let Some(arr) = payload.as_array() {
        return arr.clone();
    }
    if let Some(arr) = payload.get("messages").and_then(Value::as_array) {
        return arr.clone();
    }
    if let Some(arr) = payload.get("envelopes").and_then(Value::as_array) {
        return arr.clone();
    }
    Vec::new()
}

fn parse_destination(channel: &str) -> (&str, String) {
    if let Some(rest) = channel.strip_prefix("group:") {
        ("group", rest.to_string())
    } else if let Some(rest) = channel.strip_prefix("recipient:") {
        ("recipient", rest.to_string())
    } else {
        ("recipient", channel.to_string())
    }
}

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

pub fn poll() -> Result<Vec<(String, String)>, String> {
    let (rpc_url, account, auth_token, since) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("signal not initialized".into());
        }
        (
            s.rpc_url.clone(),
            s.account.clone(),
            s.auth_token.clone(),
            s.last_timestamp_ms,
        )
    };

    let receive_paths = [
        format!("{rpc_url}/v1/receive/{account}?timeout=1"),
        format!("{rpc_url}/v2/receive/{account}?timeout=1"),
    ];

    let mut payload = None;
    for endpoint in &receive_paths {
        match get_json(endpoint, &auth_token) {
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

    let mut outbound = Vec::new();
    let mut max_ts = since;

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

        if timestamp != 0 && timestamp <= since {
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
        outbound.push((sender, text));
    }

    if max_ts > since {
        if let Ok(mut s) = state().write() {
            s.last_timestamp_ms = max_ts;
        }
    }

    Ok(outbound)
}

pub fn send(channel: &str, text: &str) -> Result<(), String> {
    let (rpc_url, account, auth_token) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("signal not initialized".into());
        }
        (s.rpc_url.clone(), s.account.clone(), s.auth_token.clone())
    };

    let (kind, target) = parse_destination(channel);
    if target.trim().is_empty() {
        return Err("signal target channel is empty".into());
    }

    let payload = if kind == "group" {
        serde_json::json!({
            "account": account,
            "groupId": target,
            "message": text,
        })
    } else {
        serde_json::json!({
            "account": account,
            "message": text,
            "number": [target],
            "recipients": [target],
        })
    };

    let send_paths = [
        format!("{rpc_url}/v2/send"),
        format!("{rpc_url}/v1/send"),
        format!("{rpc_url}/v1/send/{account}"),
    ];

    for endpoint in &send_paths {
        match post_json(endpoint, &auth_token, &payload) {
            Ok(()) => return Ok(()),
            Err(RequestFailure::NotFound) => continue,
            Err(RequestFailure::Other(msg)) => return Err(msg),
        }
    }

    Err("signal send endpoint not found (tried /v2/send and /v1/send)".into())
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
