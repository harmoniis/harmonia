use harmonia_vault::{get_secret_for_component, init_from_env};
use serde_json::{json, Value};

use crate::model::json_escape;

#[allow(dead_code)]
const COMPONENT: &str = "parallel-agents-core";

#[allow(dead_code)]
const FALLBACK_TRUTH_KEYWORDS: &str = "truth|reality|accurate|accuracy|fact check|fact-check|verify|verification|debunk|controvers|what actually|what is really|real-time|realtime|current event|harmonic truth";

#[allow(dead_code)]
const FALLBACK_VERIFY_TEMPLATE: &str = "You are the truth-seeking verification subagent. Use live web and X search when useful. Prioritize factual accuracy over style.\n\nOriginal user prompt:\n{PROMPT}\n\nCandidate answer:\n{RESPONSE}\n\nReply exactly in this format:\nVERIFY: yes|no|uncertain\nSOURCE: web|x|web+x|unknown\nNOTES: one concise sentence";

#[allow(dead_code)]
fn extract_content_from_response(payload: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(payload).ok()?;
    harmonia_provider_protocol::extract_openai_like_content(&parsed)
}

#[allow(dead_code)]
fn extract_error_message(payload: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(payload).ok()?;
    parsed
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            parsed
                .get("message")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

#[allow(dead_code)]
fn preferred_truth_seeking_model() -> String {
    harmonia_config_store::get_config(COMPONENT, "prompts", "truth-seeking-model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "x-ai/grok-4.1-fast".to_string())
}

#[allow(dead_code)]
fn truth_seeking_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let keywords =
        harmonia_config_store::get_config(COMPONENT, "prompts", "truth-seeking-keywords")
            .ok()
            .flatten()
            .unwrap_or_else(|| FALLBACK_TRUTH_KEYWORDS.to_string());
    keywords.split('|').any(|kw| lower.contains(kw.trim()))
}

#[allow(dead_code)]
fn openrouter_payload(prompt: &str, model: &str) -> Value {
    let caps = harmonia_provider_protocol::model_capabilities(model);
    let mut payload = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
    });
    if let Some(ref r) = caps.reasoning {
        if r.enabled {
            payload["reasoning"] = json!({
                "enabled": true,
                "effort": r.effort,
                "exclude": r.exclude
            });
        }
    }
    if let Some(ref s) = caps.web_search {
        payload["plugins"] = json!([{ "id": &s.plugin_id, "engine": &s.engine }]);
        payload["web_search_options"] = json!({ "search_context_size": &s.search_context_size });
    }
    payload
}

#[allow(dead_code)]
fn clip_text(text: &str, limit: usize) -> String {
    let mut clipped = String::new();
    for ch in text.chars().take(limit) {
        clipped.push(ch);
    }
    clipped
}

#[allow(dead_code)]
fn openrouter_api_key() -> Result<Option<String>, String> {
    match get_secret_for_component("openrouter-backend", "openrouter-api-key") {
        Ok(Some(key)) => Ok(Some(key)),
        Ok(None) => get_secret_for_component("openrouter-backend", "openrouter")
            .map_err(|e| format!("vault policy error: {e}")),
        Err(e) => Err(format!("vault policy error: {e}")),
    }
}

#[allow(dead_code)]
fn make_grok_verify_prompt(prompt: &str, response: &str) -> String {
    let template = harmonia_config_store::get_config(COMPONENT, "prompts", "grok-verification")
        .ok()
        .flatten()
        .unwrap_or_else(|| FALLBACK_VERIFY_TEMPLATE.to_string());
    // Template uses {PROMPT} and {RESPONSE} placeholders (or CL-style ~A)
    template
        .replace("{PROMPT}", prompt)
        .replace("{RESPONSE}", response)
        .replacen("~A", prompt, 1)
        .replacen("~A", response, 1)
}

#[allow(dead_code)]
fn grok_verify_result(report: &str) -> (bool, String, String) {
    let lower = report.to_ascii_lowercase();
    let ok =
        lower.lines().any(|line| line.trim() == "verify: yes") || lower.contains("verify: yes");
    (ok, "grok-live".to_string(), clip_text(report.trim(), 240))
}

#[allow(dead_code)]
fn verify_with_grok(prompt: &str, response: &str) -> Result<(bool, String, String), String> {
    let key = openrouter_api_key()?.ok_or_else(|| {
        "missing secret: openrouter-api-key (vault component: openrouter-backend)".to_string()
    })?;
    let model = preferred_truth_seeking_model();
    let report = request_openrouter(&make_grok_verify_prompt(prompt, response), &model, &key)?;
    Ok(grok_verify_result(&report))
}

#[allow(dead_code)]
pub(super) fn request_openrouter(
    prompt: &str,
    model: &str,
    api_key: &str,
) -> Result<String, String> {
    let payload = openrouter_payload(prompt, model).to_string();

    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg(
            harmonia_config_store::get_config(
                COMPONENT,
                "openrouter-backend",
                "connect-timeout-secs",
            )
            .ok()
            .flatten()
            .unwrap_or_else(|| "10".to_string()),
        )
        .arg("--max-time")
        .arg(
            harmonia_config_store::get_config_or(
                COMPONENT,
                "openrouter-backend",
                "max-time-secs",
                "45",
            )
            .unwrap_or_else(|_| "45".to_string()),
        )
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
    if let Some(e) = extract_error_message(&out.stdout) {
        return Err(e);
    }
    extract_content_from_response(&out.stdout).ok_or_else(|| {
        format!(
            "missing content in response: {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })
}

#[allow(dead_code)]
fn request_exa(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = harmonia_config_store::get_config(COMPONENT, "search-exa-tool", "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.exa.ai/search".to_string());
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

#[allow(dead_code)]
fn request_brave(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = harmonia_config_store::get_config(COMPONENT, "search-brave-tool", "api-url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "https://api.search.brave.com/res/v1/web/search".to_string());
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

#[allow(dead_code)]
fn make_verify_query(prompt: &str, response: &str) -> String {
    let p = prompt.chars().take(120).collect::<String>();
    let r = response.chars().take(180).collect::<String>();
    format!("verify this answer against the web: prompt={p} answer={r}")
}

#[allow(dead_code)]
pub(super) fn verify_with_search(prompt: &str, response: &str) -> (bool, String, String) {
    let _ = init_from_env();
    if response.trim().is_empty() {
        return (false, "none".to_string(), "empty-response".to_string());
    }
    if truth_seeking_prompt(prompt) {
        match verify_with_grok(prompt, response) {
            Ok(result) => return result,
            Err(e) => {
                if let Ok(Some(exa)) =
                    get_secret_for_component("parallel-agents-core", "exa_api_key")
                {
                    let q = make_verify_query(prompt, response);
                    match request_exa(&q, &exa) {
                        Ok(payload) => {
                            let ok = payload.contains("\"results\"");
                            return (
                                ok,
                                "exa".to_string(),
                                if ok {
                                    "exa-results-found".to_string()
                                } else {
                                    format!("exa-no-results; grok={e}")
                                },
                            );
                        }
                        Err(exa_err) => {
                            if let Ok(Some(brave)) =
                                get_secret_for_component("parallel-agents-core", "brave_api_key")
                            {
                                match request_brave(&q, &brave) {
                                    Ok(payload) => {
                                        let ok = payload.contains("\"web\"")
                                            || payload.contains("\"results\"");
                                        return (
                                            ok,
                                            "brave".to_string(),
                                            if ok {
                                                "brave-results-found".to_string()
                                            } else {
                                                format!("brave-no-results; grok={e}")
                                            },
                                        );
                                    }
                                    Err(brave_err) => {
                                        return (
                                            false,
                                            "none".to_string(),
                                            format!("grok={e}; exa={exa_err}; brave={brave_err}"),
                                        );
                                    }
                                }
                            }
                            return (
                                false,
                                "none".to_string(),
                                format!("grok={e}; exa={exa_err}; brave=missing-key"),
                            );
                        }
                    }
                }
            }
        }
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
                        Err(e2) => match verify_with_grok(prompt, response) {
                            Ok(result) => return result,
                            Err(grok_err) => {
                                return (
                                    false,
                                    "none".to_string(),
                                    format!("exa={e}; brave={e2}; grok={grok_err}"),
                                );
                            }
                        },
                    }
                }
                match verify_with_grok(prompt, response) {
                    Ok(result) => return result,
                    Err(grok_err) => {
                        return (
                            false,
                            "none".to_string(),
                            format!("exa={e}; brave=missing-key; grok={grok_err}"),
                        );
                    }
                }
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
            Err(e) => match verify_with_grok(prompt, response) {
                Ok(result) => return result,
                Err(grok_err) => {
                    return (
                        false,
                        "none".to_string(),
                        format!("brave={e}; grok={grok_err}"),
                    );
                }
            },
        }
    }

    match verify_with_grok(prompt, response) {
        Ok(result) => result,
        Err(e) => (
            false,
            "none".to_string(),
            format!("missing-search-keys; grok={e}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        extract_content_from_response, grok_verify_result, openrouter_payload, truth_seeking_prompt,
    };

    #[test]
    fn extract_content_handles_openai_like_json() {
        let payload = json!({"choices":[{"message":{"content":"hello"}}]}).to_string();
        assert_eq!(
            extract_content_from_response(payload.as_bytes()).as_deref(),
            Some("hello")
        );
    }

    #[test]
    fn grok_truth_payload_enables_reasoning_and_search() {
        let payload = openrouter_payload("verify this", "x-ai/grok-4.1-fast");
        assert_eq!(payload["reasoning"]["enabled"], json!(true));
        assert_eq!(payload["plugins"][0]["id"], json!("web"));
        assert_eq!(payload["plugins"][0]["engine"], json!("native"));
        assert_eq!(
            payload["web_search_options"]["search_context_size"],
            json!("high")
        );
    }

    #[test]
    fn non_grok_payload_has_no_plugins() {
        let payload = openrouter_payload("hello", "anthropic/claude-sonnet-4.6");
        assert!(payload.get("reasoning").is_none());
        assert!(payload.get("plugins").is_none());
    }

    #[test]
    fn truth_prompt_detection_catches_accuracy_requests() {
        assert!(truth_seeking_prompt(
            "Find the harmonic truth and verify this claim."
        ));
        assert!(!truth_seeking_prompt("Write a small Rust refactor."));
    }

    #[test]
    fn grok_verify_result_reads_yes_verdict() {
        let (ok, source, detail) =
            grok_verify_result("VERIFY: yes\nSOURCE: web+x\nNOTES: matched.");
        assert!(ok);
        assert_eq!(source, "grok-live");
        assert!(detail.contains("VERIFY: yes"));
    }
}
