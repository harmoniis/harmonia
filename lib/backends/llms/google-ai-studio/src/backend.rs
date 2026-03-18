use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "google-ai-studio-backend";
const BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

pub static OFFERINGS: &[ModelOffering] = &[
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
        id: "google/gemini-2.5-flash",
        tier: "lite",
        usd_in_1k: 0.0003,
        usd_out_1k: 0.0025,
        quality: 6,
        speed: 8,
        tags: &["fast", "coding", "execution"],
    },
    ModelOffering {
        id: "google/gemini-2.5-flash-lite",
        tier: "micro",
        usd_in_1k: 0.000075,
        usd_out_1k: 0.0003,
        quality: 4,
        speed: 9,
        tags: &["fast", "memory-ops", "casual"],
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
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(
        COMPONENT,
        &[
            "google-ai-studio-api-key",
            "gemini-api-key",
            "google-api-key",
        ],
    )
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", BASE_URL)
        .unwrap_or_else(|_| BASE_URL.to_string())
}

fn normalize_model(model: &str) -> String {
    let m = strip_provider_prefix(model);
    let m = m.strip_prefix("gemini/").unwrap_or(&m);
    m.to_string()
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_GOOGLE_AI_STUDIO", 10, 60);
    let native_model = normalize_model(model);
    let url = format!(
        "{}/models/{}:generateContent?key={}",
        base_url(),
        native_model,
        api_key
    );
    let payload = json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
    });
    let headers: Vec<String> = vec![];
    let parsed = run_curl_json_post(&url, &headers, &payload, timeout)?;
    extract_google_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Google AI Studio response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "google");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "google ai studio api key missing in vault".to_string())?;
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
                "google",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "google",
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
                            "google",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "google",
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
    let key = api_key()?.ok_or_else(|| "google ai studio api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "google",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "google",
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
                            "google",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "google",
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
