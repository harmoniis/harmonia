use harmonia_vault::get_secret_for_component;
use serde_json::{json, Value};

pub(super) const COMPONENT: &str = "parallel-agents-core";

const FALLBACK_TRUTH_KEYWORDS: &str = "truth|reality|accurate|accuracy|fact check|fact-check|verify|verification|debunk|controvers|what actually|what is really|real-time|realtime|current event|harmonic truth";

fn extract_content_from_response(payload: &[u8]) -> Option<String> {
    let parsed: Value = serde_json::from_slice(payload).ok()?;
    harmonia_provider_protocol::extract_openai_like_content(&parsed)
}

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

pub(super) fn preferred_truth_seeking_model() -> String {
    harmonia_config_store::get_config(COMPONENT, "prompts", "truth-seeking-model")
        .ok()
        .flatten()
        .unwrap_or_else(|| "x-ai/grok-4.1-fast".to_string())
}

pub(super) fn truth_seeking_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    let keywords =
        harmonia_config_store::get_config(COMPONENT, "prompts", "truth-seeking-keywords")
            .ok()
            .flatten()
            .unwrap_or_else(|| FALLBACK_TRUTH_KEYWORDS.to_string());
    keywords.split('|').any(|kw| lower.contains(kw.trim()))
}

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

pub(super) fn clip_text(text: &str, limit: usize) -> String {
    let mut clipped = String::new();
    for ch in text.chars().take(limit) {
        clipped.push(ch);
    }
    clipped
}

pub(super) fn openrouter_api_key() -> Result<Option<String>, String> {
    match get_secret_for_component("openrouter-backend", "openrouter-api-key") {
        Ok(Some(key)) => Ok(Some(key)),
        Ok(None) => get_secret_for_component("openrouter-backend", "openrouter")
            .map_err(|e| format!("vault policy error: {e}")),
        Err(e) => Err(format!("vault policy error: {e}")),
    }
}

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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        extract_content_from_response, openrouter_payload, truth_seeking_prompt,
    };
    use super::super::verification::grok_verify_result;

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
