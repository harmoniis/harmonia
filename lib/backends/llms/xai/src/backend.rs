use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "xai-backend";
const DEFAULT_URL: &str = "https://api.x.ai/v1/chat/completions";

pub(crate) static OFFERINGS: &[ModelOffering] = &[
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
        id: "x-ai/grok-4",
        tier: "frontier",
        usd_in_1k: 0.003,
        usd_out_1k: 0.015,
        quality: 9,
        speed: 4,
        tags: &["frontier", "reasoning", "software-dev"],
    },
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["xai-api-key", "x-ai-api-key", "xai"])
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", DEFAULT_URL)
        .unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// Return the feature set for a normalised model name.
fn model_features(native_model: &str) -> Vec<&'static str> {
    let mut features = Vec::new();
    let lower = native_model.to_ascii_lowercase();
    if lower.starts_with("grok") {
        features.push("reasoning");
    }
    if lower.contains(":online") {
        features.push("web-search");
        features.push("x-search");
    }
    features
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_XAI", 10, 60);
    let native_model = strip_provider_prefix(model);
    let features = model_features(&native_model);

    let mut payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });

    if features.contains(&"reasoning") {
        payload
            .as_object_mut()
            .unwrap()
            .insert("reasoning".to_string(), json!({"effort": "high"}));
    }
    if features.contains(&"web-search") {
        payload
            .as_object_mut()
            .unwrap()
            .insert("plugins".to_string(), json!([{"id": "web-search"}]));
    }

    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&base_url(), &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in xAI response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub(crate) fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "xai");
    Ok(())
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "xai api key missing in vault".to_string())?;
    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, "")
    } else {
        model.to_string()
    };
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "xai",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "xai",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "xai",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "xai",
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
    let _ = init();
    let key = api_key()?.ok_or_else(|| "xai api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "xai",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "xai",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &key) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "xai",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "xai",
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

pub(crate) fn select_model(task_hint: &str) -> String {
    select_from_pool(OFFERINGS, task_hint)
}
