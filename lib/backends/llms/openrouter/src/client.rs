//! OpenRouter backend — universal gateway that routes any model via OpenRouter.
//!
//! This backend handles ALL model prefixes through OpenRouter's API. Native
//! provider backends (harmonia-openai, harmonia-anthropic, etc.) are separate
//! crates. The Lisp orchestrator tries native backends first and falls back to
//! this gateway when a native key is missing.

use harmonia_provider_protocol::*;
use serde_json::json;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const COMPONENT: &str = "openrouter-backend";

/// OpenRouter's full model catalogue available via the gateway.
/// Prices are OpenRouter passthrough prices (may include markup).
pub(crate) static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "google/gemini-3.1-flash-lite-preview",
        tier: "micro",
        usd_in_1k: 0.00025,
        usd_out_1k: 0.0015,
        quality: 3,
        speed: 9,
        tags: &["fast", "memory-ops", "casual"],
    },
    ModelOffering {
        id: "deepseek/deepseek-chat-v3.1:free",
        tier: "free",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 4,
        speed: 7,
        tags: &["reasoning", "casual"],
    },
    ModelOffering {
        id: "qwen/qwen3-coder:free",
        tier: "free",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 4,
        speed: 7,
        tags: &["coding", "execution"],
    },
    ModelOffering {
        id: "qwen/qwen3.5-flash-02-23",
        tier: "lite",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 5,
        speed: 8,
        tags: &["fast", "coding", "execution", "reasoning"],
    },
    ModelOffering {
        id: "minimax/minimax-m2.5",
        tier: "lite",
        usd_in_1k: 0.0003,
        usd_out_1k: 0.0012,
        quality: 4,
        speed: 8,
        tags: &["balanced", "memory-ops", "casual"],
    },
    ModelOffering {
        id: "amazon/nova-micro-v1",
        tier: "micro",
        usd_in_1k: 0.000035,
        usd_out_1k: 0.00014,
        quality: 2,
        speed: 9,
        tags: &["fast", "routing"],
    },
    ModelOffering {
        id: "amazon/nova-pro-v1",
        tier: "pro",
        usd_in_1k: 0.0008,
        usd_out_1k: 0.0032,
        quality: 5,
        speed: 7,
        tags: &["reasoning", "orchestration", "execution"],
    },
    ModelOffering {
        id: "moonshotai/kimi-k2.5",
        tier: "thinking",
        usd_in_1k: 0.0006,
        usd_out_1k: 0.003,
        quality: 7,
        speed: 5,
        tags: &["thinking", "software-dev", "orchestration"],
    },
    ModelOffering {
        id: "x-ai/grok-4-fast:online",
        tier: "fast-smart",
        usd_in_1k: 0.0002,
        usd_out_1k: 0.0005,
        quality: 7,
        speed: 8,
        tags: &["fast", "reasoning", "orchestration"],
    },
    ModelOffering {
        id: "google/gemini-2.5-pro",
        tier: "pro",
        usd_in_1k: 0.00125,
        usd_out_1k: 0.01,
        quality: 8,
        speed: 5,
        tags: &["reasoning", "coding", "software-dev", "orchestration"],
    },
    ModelOffering {
        id: "anthropic/claude-sonnet-4",
        tier: "pro",
        usd_in_1k: 0.003,
        usd_out_1k: 0.015,
        quality: 9,
        speed: 5,
        tags: &["reasoning", "coding", "software-dev", "orchestration"],
    },
    ModelOffering {
        id: "anthropic/claude-sonnet-4.6",
        tier: "pro",
        usd_in_1k: 0.003,
        usd_out_1k: 0.015,
        quality: 9,
        speed: 5,
        tags: &["reasoning", "coding", "software-dev", "orchestration"],
    },
    ModelOffering {
        id: "anthropic/claude-opus-4.6",
        tier: "frontier",
        usd_in_1k: 0.005,
        usd_out_1k: 0.025,
        quality: 10,
        speed: 3,
        tags: &["frontier", "reasoning", "software-dev"],
    },
    ModelOffering {
        id: "openai/gpt-5",
        tier: "frontier",
        usd_in_1k: 0.00125,
        usd_out_1k: 0.01,
        quality: 10,
        speed: 4,
        tags: &["frontier", "reasoning", "software-dev"],
    },
];

fn api_key() -> Result<String, String> {
    get_secret_any(COMPONENT, &["openrouter", "openrouter-api-key"])?
        .ok_or_else(|| "openrouter key missing in vault".to_string())
}

/// Return comma-separated feature flags for a model.
fn model_features(model: &str) -> String {
    let mut features = Vec::new();
    if model.starts_with("x-ai/grok") || model.starts_with("xai/grok") {
        features.push("reasoning");
        if model.contains(":online") {
            features.push("web-search");
            features.push("x-search");
        }
    }
    features.join(",")
}

fn request_openrouter(prompt: &str, model: &str, key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_OPENROUTER", 10, 45);
    let feats = model_features(model);
    let mut payload = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
    });
    if !feats.is_empty() {
        if feats.contains("reasoning") {
            payload["reasoning"] = json!({"effort": "high"});
        }
        if feats.contains("web-search") {
            payload["plugins"] = json!([{ "id": "web-search" }]);
        }
    }
    let headers = vec![
        format!("Authorization: Bearer {key}"),
        "HTTP-Referer: https://harmoniis.local".to_string(),
        "X-Title: Harmonia Agent".to_string(),
    ];
    let parsed = run_curl_json_post(OPENROUTER_URL, &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing OpenRouter content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub(crate) fn init_backend() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    // Register hardcoded offerings in the metrics catalogue
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "openrouter");
    // Background sync from OpenRouter API if we have a key
    if let Ok(key) = api_key() {
        std::thread::spawn(move || {
            let _ = harmonia_provider_protocol::sync_models_from_openrouter(&key);
        });
    }
    Ok(())
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init_backend();
    let key = api_key()?;
    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, "")
    } else {
        model.to_string()
    };
    let start = std::time::Instant::now();
    match request_openrouter(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "openrouter",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "openrouter",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request_openrouter(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "openrouter",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "openrouter",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            false,
                        );
                    }
                }
            }
            Err(primary_err)
        }
    }
}

pub(crate) fn complete_for_task(prompt: &str, task_hint: &str) -> Result<String, String> {
    let _ = init_backend();
    let key = api_key()?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request_openrouter(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "openrouter",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "openrouter",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request_openrouter(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "openrouter",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "openrouter",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            false,
                        );
                    }
                }
            }
            Err(primary_err)
        }
    }
}

pub(crate) fn list_offerings() -> String {
    offerings_to_json(OFFERINGS)
}

pub(crate) fn select_model_for_task(task_hint: &str) -> String {
    select_from_pool(OFFERINGS, task_hint)
}

#[cfg(test)]
mod tests {
    use harmonia_provider_protocol::*;
    use serde_json::json;

    #[test]
    fn extract_openai_style_text() {
        let v = json!({"choices":[{"message":{"content":"hello"}}]});
        assert_eq!(extract_openai_like_content(&v).as_deref(), Some("hello"));
    }
}
