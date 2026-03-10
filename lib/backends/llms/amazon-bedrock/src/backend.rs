use std::process::Command;

use harmonia_provider_protocol::*;
use serde_json::json;

const COMPONENT: &str = "amazon-bedrock-backend";

pub(crate) static OFFERINGS: &[ModelOffering] = &[
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
        id: "amazon/nova-lite-v1",
        tier: "lite",
        usd_in_1k: 0.00006,
        usd_out_1k: 0.00024,
        quality: 3,
        speed: 8,
        tags: &["fast", "vision", "cheap"],
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
];

fn aws_access_key_id() -> Option<String> {
    get_secret_any(COMPONENT, &["aws-access-key-id"])
        .ok()
        .flatten()
}

fn aws_secret_access_key() -> Option<String> {
    get_secret_any(COMPONENT, &["aws-secret-access-key"])
        .ok()
        .flatten()
}

fn aws_session_token() -> Option<String> {
    get_secret_any(COMPONENT, &["aws-session-token"])
        .ok()
        .flatten()
}

fn aws_region() -> String {
    if let Ok(Some(v)) = harmonia_config_store::get_own(COMPONENT, "region") {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }

    // Legacy compatibility: move old vault region to config-store.
    if let Ok(Some(v)) = get_secret_any(COMPONENT, &["aws-region"]) {
        if !v.trim().is_empty() {
            let trimmed = v.trim().to_string();
            let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "region", &trimmed);
            return trimmed;
        }
    }

    "us-east-1".to_string()
}

fn normalize_model(model: &str) -> String {
    let m = strip_provider_prefix(model);
    let m = m.replace('/', ".");
    if m.contains(':') {
        m
    } else {
        format!("{m}:0")
    }
}

fn request(prompt: &str, model: &str) -> Result<String, String> {
    let native_model = normalize_model(model);
    let region = aws_region();
    let messages = json!([{"role": "user", "content": [{"text": prompt}]}]);
    let messages_str =
        serde_json::to_string(&messages).map_err(|e| format!("json encode failed: {e}"))?;

    let mut cmd = Command::new("aws");
    cmd.arg("bedrock-runtime")
        .arg("converse")
        .arg("--model-id")
        .arg(&native_model)
        .arg("--messages")
        .arg(&messages_str)
        .arg("--region")
        .arg(&region);

    if let Some(key_id) = aws_access_key_id() {
        cmd.env("AWS_ACCESS_KEY_ID", key_id);
    }
    if let Some(secret) = aws_secret_access_key() {
        cmd.env("AWS_SECRET_ACCESS_KEY", secret);
    }
    if let Some(token) = aws_session_token() {
        cmd.env("AWS_SESSION_TOKEN", token);
    }
    cmd.env("AWS_REGION", &region);

    let output = cmd
        .output()
        .map_err(|e| format!("aws cli exec failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!(
            "aws bedrock-runtime converse failed: {}",
            clip(&stderr, 320)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed = parse_json_response(&stdout)?;
    extract_bedrock_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in Bedrock response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

pub(crate) fn init() -> Result<(), String> {
    harmonia_provider_protocol::harmonia_vault::init_from_env()?;
    harmonia_provider_protocol::upsert_hardcoded_offerings(OFFERINGS, "amazon");
    Ok(())
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init();
    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, "")
    } else {
        model.to_string()
    };
    let start = std::time::Instant::now();
    match request(prompt, &selected) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "amazon",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "amazon",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "amazon",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "amazon",
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
    let selected = select_from_pool(OFFERINGS, task_hint);
    let start = std::time::Instant::now();
    match request(prompt, &selected) {
        Ok(text) => {
            log_model_performance(
                OFFERINGS,
                "amazon",
                &selected,
                start.elapsed().as_millis(),
                true,
            );
            Ok(text)
        }
        Err(primary_err) => {
            log_model_performance(
                OFFERINGS,
                "amazon",
                &selected,
                start.elapsed().as_millis(),
                false,
            );
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_start = std::time::Instant::now();
                match request(prompt, &fb) {
                    Ok(text) => {
                        log_model_performance(
                            OFFERINGS,
                            "amazon",
                            &fb,
                            fb_start.elapsed().as_millis(),
                            true,
                        );
                        return Ok(text);
                    }
                    Err(_) => {
                        log_model_performance(
                            OFFERINGS,
                            "amazon",
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
