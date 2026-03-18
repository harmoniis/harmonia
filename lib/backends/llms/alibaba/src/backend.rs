use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "alibaba-backend";
const DEFAULT_URL: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions";

pub static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "qwen/qwen3.5-flash-02-23",
        tier: "lite",
        usd_in_1k: 0.0001,
        usd_out_1k: 0.0004,
        quality: 5,
        speed: 8,
        tags: &["fast", "coding", "execution", "reasoning"],
    },
    ModelOffering {
        id: "qwen/qwen3-coder",
        tier: "lite",
        usd_in_1k: 0.00022,
        usd_out_1k: 0.001,
        quality: 4,
        speed: 7,
        tags: &["coding", "execution"],
    },
    ModelOffering {
        id: "qwen/qwen-plus",
        tier: "pro",
        usd_in_1k: 0.0004,
        usd_out_1k: 0.0012,
        quality: 7,
        speed: 6,
        tags: &["reasoning", "orchestration"],
    },
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(
        COMPONENT,
        &["alibaba-api-key", "dashscope-api-key", "alibaba"],
    )
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", DEFAULT_URL)
        .unwrap_or_else(|_| DEFAULT_URL.to_string())
}

/// Normalize model id: strip "qwen/", "alibaba/", or "dashscope/" prefix to get the
/// native model name expected by the DashScope API.
fn normalize_model(model: &str) -> String {
    let trimmed = model.trim();
    for prefix in &["qwen/", "alibaba/", "dashscope/"] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return rest.to_string();
        }
    }
    trimmed.to_string()
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_ALIBABA", 10, 60);
    let native_model = normalize_model(model);
    let payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&base_url(), &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Alibaba/DashScope response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "alibaba");
    Ok(())
}

pub fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "alibaba api key missing in vault".to_string())?;
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
                "alibaba",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "alibaba",
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
                            "alibaba",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "alibaba",
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
    let key = api_key()?.ok_or_else(|| "alibaba api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "alibaba",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "alibaba",
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
                            "alibaba",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "alibaba",
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
