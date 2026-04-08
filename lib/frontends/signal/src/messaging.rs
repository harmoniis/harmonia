use crate::rpc::{
    extract_events, extract_first_string, extract_first_u64, get_json, parse_destination,
    post_json, RequestFailure,
};
use crate::state::state;

pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
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
        let metadata = format!(
            "(:channel-class \"signal-bridge\" :node-id \"{}\" :remote t)",
            sender.replace('\\', "\\\\").replace('"', "\\\"")
        );
        outbound.push((sender, text, Some(metadata)));
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
