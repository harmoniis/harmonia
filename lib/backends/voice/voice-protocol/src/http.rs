use std::io::Read;

pub struct TimeoutConfig {
    pub connect_secs: u64,
    pub max_secs: u64,
}

pub fn get_timeout(
    component: &str,
    _env_prefix: &str,
    default_connect: u64,
    default_max: u64,
) -> TimeoutConfig {
    let connect = harmonia_config_store::get_own(component, "connect-timeout-secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_connect);
    let max = harmonia_config_store::get_own(component, "max-time-secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default_max);
    TimeoutConfig {
        connect_secs: connect,
        max_secs: max,
    }
}

pub fn get_secret_any(component: &str, symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env().ok();
    for sym in symbols {
        match harmonia_vault::get_secret_for_component(component, sym) {
            Ok(Some(v)) if !v.is_empty() => return Ok(Some(v)),
            _ => continue,
        }
    }
    Ok(None)
}

pub fn clip(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

pub fn ureq_post_json(
    url: &str,
    headers: &[(String, String)],
    body: &serde_json::Value,
    timeout: &TimeoutConfig,
) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(timeout.connect_secs))
        .timeout(std::time::Duration::from_secs(timeout.max_secs))
        .build();

    let mut req = agent.post(url);
    req = req.set("Content-Type", "application/json");
    for (key, value) in headers {
        req = req.set(key, value);
    }

    let resp = req
        .send_string(&body.to_string())
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let mut body_str = String::new();
    resp.into_reader()
        .take(4 * 1024 * 1024)
        .read_to_string(&mut body_str)
        .map_err(|e| format!("failed to read response: {e}"))?;

    serde_json::from_str(&body_str)
        .map_err(|e| format!("invalid JSON response: {e} body={}", clip(&body_str, 320)))
}

pub fn ureq_post_multipart(
    url: &str,
    api_key: &str,
    fields: &[(&str, &str)],
    file_field: &str,
    file_path: &str,
    timeout: &TimeoutConfig,
) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(timeout.connect_secs))
        .timeout(std::time::Duration::from_secs(timeout.max_secs))
        .build();

    let file_data =
        std::fs::read(file_path).map_err(|e| format!("cannot read file {file_path}: {e}"))?;

    let file_name = std::path::Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.wav");

    let boundary = format!(
        "harmonia-{:016x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );

    let mut body = Vec::new();
    for (key, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{file_field}\"; filename=\"{file_name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&file_data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    let resp = agent
        .post(url)
        .set("Authorization", &format!("Bearer {api_key}"))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .send_bytes(&body)
        .map_err(|e| format!("HTTP multipart request failed: {e}"))?;

    let mut resp_body = String::new();
    resp.into_reader()
        .take(4 * 1024 * 1024)
        .read_to_string(&mut resp_body)
        .map_err(|e| format!("failed to read response: {e}"))?;

    Ok(resp_body)
}
