use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "anthropic-backend";
const URL: &str = "https://api.anthropic.com/v1/messages";

pub(crate) static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "anthropic/claude-haiku-4.5",
        tier: "lite",
        usd_in_1k: 0.00025,
        usd_out_1k: 0.00125,
        quality: 6,
        speed: 8,
        tags: &["fast", "execution", "memory-ops"],
    },
    ModelOffering {
        id: "anthropic/claude-sonnet-4",
        tier: "pro",
        usd_in_1k: 0.003,
        usd_out_1k: 0.015,
        quality: 8,
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
        id: "anthropic/claude-opus-4",
        tier: "frontier",
        usd_in_1k: 0.005,
        usd_out_1k: 0.025,
        quality: 10,
        speed: 3,
        tags: &["frontier", "reasoning", "software-dev"],
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
];

fn api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["anthropic-api-key", "anthropic"])
}

fn api_version() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "api-version", "2023-06-01")
        .unwrap_or_else(|_| "2023-06-01".to_string())
}

fn max_tokens() -> u64 {
    harmonia_config_store::get_own(COMPONENT, "max-tokens")
        .ok()
        .flatten()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1024)
}

fn request(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_ANTHROPIC", 10, 60);
    let native_model = strip_provider_prefix(model);
    let payload = json!({
        "model": native_model,
        "max_tokens": max_tokens(),
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![
        format!("x-api-key: {api_key}"),
        format!("anthropic-version: {}", api_version()),
    ];
    let parsed = run_curl_json_post(URL, &headers, &payload, timeout)?;
    extract_anthropic_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Anthropic response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub(crate) fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "anthropic");
    Ok(())
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let key = api_key()?.ok_or_else(|| "anthropic api key missing in vault".to_string())?;
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
                "anthropic",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "anthropic",
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
                            "anthropic",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "anthropic",
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
    let key = api_key()?.ok_or_else(|| "anthropic api key missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &key) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "anthropic",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "anthropic",
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
                            "anthropic",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "anthropic",
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
