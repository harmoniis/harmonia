use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "xai-backend";
const DEFAULT_URL: &str = "https://api.x.ai/v1/chat/completions";

pub static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "x-ai/grok-4.1-fast",
        tier: "fast-smart",
        usd_in_1k: 0.0002,
        usd_out_1k: 0.0005,
        quality: 7,
        speed: 8,
        tags: &[
            "fast",
            "reasoning",
            "truth-seeking",
            "web-search",
            "x-search",
            "realtime",
        ],
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

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_XAI", 10, 60);
    let native_model = strip_provider_prefix(model);
    let caps = model_capabilities(model);

    let mut payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });

    if let Some(ref r) = caps.reasoning {
        if r.enabled {
            payload.as_object_mut().unwrap().insert(
                "reasoning".to_string(),
                json!({"enabled": true, "effort": r.effort, "exclude": r.exclude}),
            );
        }
    }
    if let Some(ref s) = caps.web_search {
        payload.as_object_mut().unwrap().insert(
            "plugins".to_string(),
            json!([{"id": &s.plugin_id, "engine": &s.engine}]),
        );
        payload.as_object_mut().unwrap().insert(
            "web_search_options".to_string(),
            json!({"search_context_size": &s.search_context_size}),
        );
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

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "xai");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
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

pub fn complete_for_task(prompt: &str, task_hint: &str) -> Result<String, String> {
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

pub fn list_offerings() -> String {
    offerings_to_json(OFFERINGS)
}

pub fn select_model(task_hint: &str) -> String {
    select_from_pool(OFFERINGS, task_hint)
}
