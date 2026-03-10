use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "google-vertex-backend";

pub(crate) static OFFERINGS: &[ModelOffering] = &[
    ModelOffering {
        id: "vertex/gemini-3.1-flash-lite-preview",
        tier: "micro",
        usd_in_1k: 0.00025,
        usd_out_1k: 0.0015,
        quality: 3,
        speed: 9,
        tags: &["fast", "memory-ops", "casual"],
    },
    ModelOffering {
        id: "vertex/gemini-2.5-flash",
        tier: "lite",
        usd_in_1k: 0.0003,
        usd_out_1k: 0.0025,
        quality: 6,
        speed: 8,
        tags: &["fast", "coding", "execution"],
    },
    ModelOffering {
        id: "vertex/gemini-2.5-pro",
        tier: "pro",
        usd_in_1k: 0.00125,
        usd_out_1k: 0.01,
        quality: 8,
        speed: 5,
        tags: &["reasoning", "coding", "software-dev", "orchestration"],
    },
];

fn access_token() -> Result<Option<String>, String> {
    get_secret_any(
        COMPONENT,
        &["google-vertex-access-token", "vertex-access-token"],
    )
}

fn project_id() -> Result<String, String> {
    if let Some(v) = harmonia_config_store::get_own(COMPONENT, "project-id")
        .ok()
        .flatten()
    {
        if !v.trim().is_empty() {
            return Ok(v);
        }
    }
    match get_secret_any(
        COMPONENT,
        &["google-vertex-project-id", "vertex-project-id"],
    )? {
        Some(v) if !v.trim().is_empty() => Ok(v),
        _ => Err("google vertex project id missing".to_string()),
    }
}

fn location() -> String {
    if let Some(v) = harmonia_config_store::get_own(COMPONENT, "location")
        .ok()
        .flatten()
    {
        if !v.trim().is_empty() {
            return v;
        }
    }
    match get_secret_any(COMPONENT, &["google-vertex-location", "vertex-location"]) {
        Ok(Some(v)) if !v.trim().is_empty() => v,
        _ => "us-central1".to_string(),
    }
}

fn endpoint(location: &str) -> String {
    harmonia_config_store::get_own(COMPONENT, "endpoint")
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("https://{location}-aiplatform.googleapis.com"))
}

fn normalize_model(model: &str) -> String {
    let m = strip_provider_prefix(model);
    let m = m.strip_prefix("google-vertex/").unwrap_or(&m);
    m.to_string()
}

fn request(prompt: &str, model: &str, token: &str) -> Result<String, String> {
    let timeout = get_timeout(COMPONENT, "HARMONIA_GOOGLE_VERTEX", 10, 60);
    let native_model = normalize_model(model);
    let loc = location();
    let proj = project_id()?;
    let ep = endpoint(&loc);
    let url = format!(
        "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        ep, proj, loc, native_model
    );
    let payload = json!({
        "contents": [{"role": "user", "parts": [{"text": prompt}]}],
    });
    let headers = vec![format!("Authorization: Bearer {token}")];
    let parsed = run_curl_json_post(&url, &headers, &payload, timeout)?;
    extract_google_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Google Vertex response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub(crate) fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "vertex");
    Ok(())
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let token =
        access_token()?.ok_or_else(|| "google vertex access token missing in vault".to_string())?;
    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, "")
    } else {
        model.to_string()
    };
    let start = std::time::Instant::now();
    match request(prompt, &selected, &token) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "vertex",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "vertex",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &token) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "vertex",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "vertex",
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
    let token =
        access_token()?.ok_or_else(|| "google vertex access token missing in vault".to_string())?;
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected, &token) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "vertex",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "vertex",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb, &token) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "vertex",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "vertex",
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
