use serde_json::Value;

pub(crate) enum RequestFailure {
    NotFound,
    Other(String),
}

pub(crate) fn apply_auth(req: ureq::Request, auth_token: &str) -> ureq::Request {
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

pub(crate) fn get_json(url: &str, auth_token: &str) -> Result<Value, RequestFailure> {
    apply_auth(ureq::get(url), auth_token)
        .call()
        .map_err(error_from_ureq)?
        .into_json()
        .map_err(|e| RequestFailure::Other(format!("signal json decode failed: {e}")))
}

pub(crate) fn post_json(url: &str, auth_token: &str, body: &Value) -> Result<(), RequestFailure> {
    apply_auth(ureq::post(url), auth_token)
        .set("Content-Type", "application/json")
        .send_json(body)
        .map_err(error_from_ureq)?;
    Ok(())
}

pub(crate) fn get_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub(crate) fn extract_first_string(root: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(v) = get_path(root, path).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub(crate) fn extract_first_u64(root: &Value, paths: &[&[&str]]) -> Option<u64> {
    for path in paths {
        if let Some(v) = get_path(root, path).and_then(Value::as_u64) {
            return Some(v);
        }
    }
    None
}

pub(crate) fn extract_events(payload: Value) -> Vec<Value> {
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

pub(crate) fn parse_destination(channel: &str) -> (&str, String) {
    if let Some(rest) = channel.strip_prefix("group:") {
        ("group", rest.to_string())
    } else if let Some(rest) = channel.strip_prefix("recipient:") {
        ("recipient", rest.to_string())
    } else {
        ("recipient", channel.to_string())
    }
}
