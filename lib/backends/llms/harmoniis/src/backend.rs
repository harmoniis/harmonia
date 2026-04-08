//! Harmoniis backend — native Harmoniis inference provider.
//!
//! Models:
//!   - ber1-ai/qwen3.5-27b   — reasoning, orchestration, coding
//!   - ber1-ai/magistral-24b — reasoning, orchestration, planning
//!   - ber1-ai/nanbeige-3b   — fast execution, subagent tasks
//!
//! OpenAI-compatible API. Qwen3.5-27B and Magistral support reasoning
//! natively via ChatML `<think>` blocks. Nanbeige is optimised for speed.

use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "harmoniis-backend";
const DEFAULT_URL: &str = "https://router.harmoniis.com/v1/chat/completions";

/// Harmoniis model catalogue.
pub static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "ber1-ai/qwen3.5-27b",
        tier: "free",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 7,
        speed: 5,
        tags: &[
            "reasoning",
            "orchestration",
            "software-dev",
            "coding",
            "planning",
            "structured-output",
        ],
    },
    ModelOffering {
        id: "ber1-ai/magistral-24b",
        tier: "free",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 6,
        speed: 4,
        tags: &[
            "reasoning",
            "orchestration",
            "planning",
            "software-dev",
        ],
    },
    ModelOffering {
        id: "ber1-ai/nanbeige-3b",
        tier: "free",
        usd_in_1k: 0.0,
        usd_out_1k: 0.0,
        quality: 4,
        speed: 9,
        tags: &[
            "fast",
            "execution",
            "memory-ops",
            "casual",
            "structured-output",
        ],
    },
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["harmoniis-api-key", "harmoniis-router-api-key", "harmoniis"])
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", DEFAULT_URL)
        .unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// Build the request payload with reasoning support.
/// Qwen3.5-27B and Magistral natively support reasoning via ChatML `<think>` blocks.
/// The llama-server returns `reasoning_content` in the OpenAI-compatible response.
/// When reasoning is enabled in capabilities, we set temperature=0.6 (recommended for
/// thinking models) and pass max_tokens high enough for both reasoning + answer.
fn request_payload(prompt: &str, model: &str) -> serde_json::Value {
    let caps = model_capabilities(model);
    let native_model = strip_provider_prefix(model);

    let mut payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });

    if let Some(ref r) = caps.reasoning {
        if r.enabled {
            // Reasoning models need higher token budget for thinking + answer.
            // Temperature 0.6 is the Qwen3.5 recommended setting for reasoning.
            payload["temperature"] = json!(0.6);
            payload["max_tokens"] = json!(4096);
        }
    } else {
        // Non-reasoning models (Nanbeige): deterministic, concise output.
        payload["temperature"] = json!(0.0);
        payload["max_tokens"] = json!(2048);
    }

    payload
}

/// Per-model timeout. Reasoning models get longer timeouts (they think before answering).
fn model_timeout(model: &str) -> (u64, u64) {
    if model.contains("nanbeige") || model.contains("3b") {
        (5, 30) // fast model: 30s max
    } else {
        (10, 120) // reasoning models: 120s max (thinking takes time)
    }
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let (connect, max_time) = model_timeout(model);
    let timeout = TimeoutConfig {
        connect_timeout_secs: connect,
        max_time_secs: max_time,
    };
    let payload = request_payload(prompt, model);
    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&base_url(), &headers, &payload, timeout)?;

    // llama-server returns OpenAI-compatible responses.
    // For reasoning models, the answer is in `content` (reasoning in `reasoning_content`).
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Harmoniis response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "harmoniis");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "harmoniis api key missing in vault".to_string())?;
    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, "")
    } else {
        model.to_string()
    };
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(OFFERINGS, "harmoniis", &selected, start.elapsed().as_millis(), true);
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(OFFERINGS, "harmoniis", &selected, start.elapsed().as_millis(), false);
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(OFFERINGS, "harmoniis", &fb, fb_start.elapsed().as_millis(), true);
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(OFFERINGS, "harmoniis", &fb, fb_start.elapsed().as_millis(), false);
                    }
                }
            }
            Err(primary_err)
        }
    }
}

pub fn complete_for_task(prompt: &str, task_hint: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "harmoniis api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(OFFERINGS, "harmoniis", &selected, start.elapsed().as_millis(), true);
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(OFFERINGS, "harmoniis", &selected, start.elapsed().as_millis(), false);
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(OFFERINGS, "harmoniis", &fb, fb_start.elapsed().as_millis(), true);
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(OFFERINGS, "harmoniis", &fb, fb_start.elapsed().as_millis(), false);
                    }
                }
            }
            Err(primary_err)
        }
    }
}

pub fn list_offerings() -> String {
    offerings_to_json(OFFERINGS)
}

pub fn select_model(task_hint: &str) -> String {
    select_from_pool(OFFERINGS, task_hint)
}
