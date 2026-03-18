use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "openai-backend";
const DEFAULT_URL: &str = "https://api.openai.com/v1/chat/completions";

pub static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "openai/gpt-4.1-nano",
        tier: "micro",
        usd_in_1k: 0.00002,
        usd_out_1k: 0.00015,
        quality: 4,
        speed: 9,
        tags: &["fast", "memory-ops", "routing"],
    },
    ModelOffering {
        id: "openai/gpt-4.1-mini",
        tier: "lite",
        usd_in_1k: 0.0004,
        usd_out_1k: 0.0016,
        quality: 6,
        speed: 7,
        tags: &["fast", "coding", "execution"],
    },
    ModelOffering {
        id: "openai/gpt-4.1",
        tier: "pro",
        usd_in_1k: 0.002,
        usd_out_1k: 0.008,
        quality: 8,
        speed: 5,
        tags: &["reasoning", "coding", "software-dev", "orchestration"],
    },
    ModelOffering {
        id: "openai/o4-mini",
        tier: "lite",
        usd_in_1k: 0.0011,
        usd_out_1k: 0.0044,
        quality: 7,
        speed: 7,
        tags: &["reasoning", "coding", "execution"],
    },
    ModelOffering {
        id: "openai/o3",
        tier: "pro",
        usd_in_1k: 0.002,
        usd_out_1k: 0.008,
        quality: 9,
        speed: 4,
        tags: &["reasoning", "software-dev", "orchestration"],
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

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["openai-api-key", "openai"])
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", DEFAULT_URL)
        .unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_OPENAI", 10, 60);
    let native_model = strip_provider_prefix(model);
    let payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&base_url(), &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in OpenAI response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "openai");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "openai api key missing in vault".to_string())?;
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
                "openai",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "openai",
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
                            "openai",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "openai",
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
    let key = api_key()?.ok_or_else(|| "openai api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "openai",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "openai",
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
                            "openai",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "openai",
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
