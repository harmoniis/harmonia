use std::env;

use harmonia_vault::{get_secret_for_component, init_from_env};

use crate::model::json_escape;

fn extract_content_from_response(payload: &str) -> Option<String> {
    let key = "\"content\":\"";
    let start = payload.find(key)?;
    let rest = &payload[start + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_error_message(payload: &str) -> Option<String> {
    let key = "\"message\":\"";
    let start = payload.find(key)?;
    let rest = &payload[start + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub(super) fn request_openrouter(
    prompt: &str,
    model: &str,
    api_key: &str,
) -> Result<String, String> {
    let payload = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
        json_escape(model),
        json_escape(prompt)
    );

    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg(
            env::var("HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".to_string()),
        )
        .arg("--max-time")
        .arg(env::var("HARMONIA_OPENROUTER_MAX_TIME_SECS").unwrap_or_else(|_| "45".to_string()))
        .arg("-X")
        .arg("POST")
        .arg("https://openrouter.ai/api/v1/chat/completions")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg("HTTP-Referer: https://harmoniis.local")
        .arg("-H")
        .arg("X-Title: Harmonia Parallel Agents")
        .arg("-d")
        .arg(payload)
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if let Some(e) = extract_error_message(&stdout) {
        return Err(e);
    }
    extract_content_from_response(&stdout)
        .ok_or_else(|| format!("missing content in response: {stdout}"))
}

fn request_exa(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_EXA_API_URL")
        .unwrap_or_else(|_| "https://api.exa.ai/search".to_string());
    let payload = format!("{{\"query\":\"{}\",\"numResults\":5}}", json_escape(query));
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("x-api-key: {api_key}"))
        .arg("-d")
        .arg(payload)
        .arg(endpoint)
        .output()
        .map_err(|e| format!("exa curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "exa curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn request_brave(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_BRAVE_API_URL")
        .unwrap_or_else(|_| "https://api.search.brave.com/res/v1/web/search".to_string());
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-G")
        .arg("-H")
        .arg(format!("X-Subscription-Token: {api_key}"))
        .arg("--data-urlencode")
        .arg(format!("q={query}"))
        .arg(endpoint)
        .output()
        .map_err(|e| format!("brave curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "brave curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn make_verify_query(prompt: &str, response: &str) -> String {
    let p = prompt.chars().take(120).collect::<String>();
    let r = response.chars().take(180).collect::<String>();
    format!("verify this answer against the web: prompt={p} answer={r}")
}

pub(super) fn verify_with_search(prompt: &str, response: &str) -> (bool, String, String) {
    let _ = init_from_env();
    if response.trim().is_empty() {
        return (false, "none".to_string(), "empty-response".to_string());
    }
    let q = make_verify_query(prompt, response);

    if let Ok(Some(exa)) = get_secret_for_component("parallel-agents-core", "exa_api_key") {
        match request_exa(&q, &exa) {
            Ok(payload) => {
                let ok = payload.contains("\"results\"");
                return (
                    ok,
                    "exa".to_string(),
                    if ok {
                        "exa-results-found".to_string()
                    } else {
                        "exa-no-results".to_string()
                    },
                );
            }
            Err(e) => {
                if let Ok(Some(brave)) =
                    get_secret_for_component("parallel-agents-core", "brave_api_key")
                {
                    match request_brave(&q, &brave) {
                        Ok(payload) => {
                            let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                            return (
                                ok,
                                "brave".to_string(),
                                if ok {
                                    "brave-results-found".to_string()
                                } else {
                                    "brave-no-results".to_string()
                                },
                            );
                        }
                        Err(e2) => {
                            return (false, "none".to_string(), format!("exa={e}; brave={e2}"));
                        }
                    }
                }
                return (
                    false,
                    "none".to_string(),
                    format!("exa={e}; brave=missing-key"),
                );
            }
        }
    }

    if let Ok(Some(brave)) = get_secret_for_component("parallel-agents-core", "brave_api_key") {
        match request_brave(&q, &brave) {
            Ok(payload) => {
                let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                return (
                    ok,
                    "brave".to_string(),
                    if ok {
                        "brave-results-found".to_string()
                    } else {
                        "brave-no-results".to_string()
                    },
                );
            }
            Err(e) => return (false, "none".to_string(), format!("brave={e}")),
        }
    }

    (false, "none".to_string(), "missing-search-keys".to_string())
}
