use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "groq-backend";
const DEFAULT_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

pub static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "groq/llama-3.3-70b-versatile",
        tier: "lite",
        usd_in_1k: 0.00059,
        usd_out_1k: 0.00079,
        quality: 6,
        speed: 9,
        tags: &["fast", "execution", "casual"],
    },
    ModelOffering {
        id: "groq/llama-4-scout-17b-16e-instruct",
        tier: "lite",
        usd_in_1k: 0.00011,
        usd_out_1k: 0.00034,
        quality: 5,
        speed: 9,
        tags: &["fast", "coding", "memory-ops"],
    },
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["groq-api-key", "groq"])
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", DEFAULT_URL)
        .unwrap_or_else(|_| DEFAULT_URL.to_string())
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_GROQ", 10, 60);
    let native_model = strip_provider_prefix(model);
    let payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&base_url(), &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Groq response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "groq");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "groq api key missing in vault".to_string())?;
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
                "groq",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "groq",
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
                            "groq",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "groq",
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
    let key = api_key()?.ok_or_else(|| "groq api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "groq",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "groq",
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
                            "groq",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "groq",
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
