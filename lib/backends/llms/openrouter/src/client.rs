use std::env;
use std::process::Command;

use harmonia_vault::{get_secret_for_component, init_from_env};
use serde_json::{json, Value};

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const OPENAI_URL: &str = "https://api.openai.com/v1/chat/completions";
const XAI_URL: &str = "https://api.x.ai/v1/chat/completions";
const GROQ_URL: &str = "https://api.groq.com/openai/v1/chat/completions";
const ALIBABA_URL: &str = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1/chat/completions";
const ANTHROPIC_URL: &str = "https://api.anthropic.com/v1/messages";
const GOOGLE_AI_STUDIO_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    OpenRouter,
    OpenAI,
    Anthropic,
    Xai,
    GoogleAiStudio,
    GoogleVertex,
    AmazonBedrock,
    Groq,
    Alibaba,
}

#[derive(Debug, Clone)]
struct TimeoutConfig {
    connect_timeout_secs: u64,
    max_time_secs: u64,
}

fn bool_env(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn clip(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut out = text.chars().take(limit).collect::<String>();
    out.push_str("...");
    out
}

fn get_timeout(prefix: &str, connect_default: u64, max_default: u64) -> TimeoutConfig {
    let connect_name = format!("{prefix}_CONNECT_TIMEOUT_SECS");
    let max_name = format!("{prefix}_MAX_TIME_SECS");
    TimeoutConfig {
        connect_timeout_secs: env::var(&connect_name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(connect_default),
        max_time_secs: env::var(&max_name)
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(max_default),
    }
}

fn provider_from_model(model: &str) -> Provider {
    let head = model
        .trim()
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match head.as_str() {
        "openai" => Provider::OpenAI,
        "anthropic" => Provider::Anthropic,
        "xai" | "x-ai" => Provider::Xai,
        "google" | "gemini" | "google-ai-studio" => Provider::GoogleAiStudio,
        "vertex" | "google-vertex" => Provider::GoogleVertex,
        "amazon" | "bedrock" | "nova" => Provider::AmazonBedrock,
        "groq" => Provider::Groq,
        "alibaba" | "dashscope" | "qwen" => Provider::Alibaba,
        "openrouter" => Provider::OpenRouter,
        _ => Provider::OpenRouter,
    }
}

fn normalize_model_for_provider(provider: Provider, raw_model: &str) -> String {
    let trimmed = raw_model.trim();
    let base = trimmed
        .split_once('/')
        .map(|(_, tail)| tail.to_string())
        .unwrap_or_else(|| trimmed.to_string());
    let no_variant = base.split(':').next().unwrap_or("").trim().to_string();
    match provider {
        Provider::OpenRouter => trimmed.to_string(),
        Provider::AmazonBedrock => {
            // Map "amazon/nova-pro-v1" -> "amazon.nova-pro-v1:0" for Bedrock Converse API.
            if no_variant.is_empty() {
                no_variant
            } else if no_variant.contains('.') {
                if no_variant.contains(':') {
                    no_variant
                } else {
                    format!("{no_variant}:0")
                }
            } else if no_variant.contains(':') {
                format!("amazon.{no_variant}")
            } else {
                format!("amazon.{no_variant}:0")
            }
        }
        Provider::Xai => no_variant,
        Provider::OpenAI => no_variant,
        Provider::Anthropic => no_variant,
        Provider::GoogleAiStudio => no_variant,
        Provider::GoogleVertex => no_variant,
        Provider::Groq => no_variant,
        Provider::Alibaba => no_variant,
    }
}

fn default_model() -> Result<String, String> {
    for key in [
        "HARMONIA_LLM_DEFAULT_MODEL",
        "HARMONIA_MODEL_DEFAULT",
        "HARMONIA_OPENROUTER_DEFAULT_MODEL",
    ] {
        if let Ok(v) = env::var(key) {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }
    if let Some(v) = fallback_models().into_iter().next() {
        return Ok(v);
    }
    Err("no default model provided (set model in request or env HARMONIA_LLM_DEFAULT_MODEL/HARMONIA_MODEL_DEFAULT/HARMONIA_OPENROUTER_DEFAULT_MODEL)".to_string())
}

fn fallback_models() -> Vec<String> {
    let raw = env::var("HARMONIA_LLM_FALLBACK_MODELS")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| env::var("HARMONIA_OPENROUTER_FALLBACK_MODELS").unwrap_or_default());
    raw.split(',')
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect()
}

fn get_secret_any(component: &str, symbols: &[&str]) -> Result<Option<String>, String> {
    for symbol in symbols {
        let got = get_secret_for_component(component, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(v) = got {
            if !v.trim().is_empty() {
                return Ok(Some(v));
            }
        }
    }
    Ok(None)
}

fn openrouter_key() -> Result<Option<String>, String> {
    get_secret_any("openrouter-backend", &["openrouter", "openrouter-api-key"])
}

fn provider_key(provider: Provider) -> Result<Option<String>, String> {
    match provider {
        Provider::OpenRouter => openrouter_key(),
        Provider::OpenAI => get_secret_any("openai-backend", &["openai-api-key", "openai"]),
        Provider::Anthropic => {
            get_secret_any("anthropic-backend", &["anthropic-api-key", "anthropic"])
        }
        Provider::Xai => get_secret_any("xai-backend", &["xai-api-key", "x-ai-api-key", "xai"]),
        Provider::GoogleAiStudio => get_secret_any(
            "google-ai-studio-backend",
            &[
                "google-ai-studio-api-key",
                "gemini-api-key",
                "google-api-key",
            ],
        ),
        Provider::GoogleVertex => get_secret_any(
            "google-vertex-backend",
            &["google-vertex-access-token", "vertex-access-token"],
        ),
        Provider::Groq => get_secret_any("groq-backend", &["groq-api-key", "groq"]),
        Provider::Alibaba => get_secret_any(
            "alibaba-backend",
            &["alibaba-api-key", "dashscope-api-key", "alibaba"],
        ),
        Provider::AmazonBedrock => Ok(Some(String::new())),
    }
}

fn openrouter_fallback_enabled() -> bool {
    !bool_env("HARMONIA_LLM_DISABLE_OPENROUTER_FALLBACK", false)
}

fn parse_json_response(stdout: &str) -> Result<Value, String> {
    if stdout.trim().is_empty() {
        return Err("empty response".to_string());
    }
    serde_json::from_str(stdout)
        .map_err(|e| format!("invalid JSON response: {e}; body={}", clip(stdout, 320)))
}

fn json_error_message(v: &Value) -> Option<String> {
    if let Some(msg) = v
        .get("error")
        .and_then(|x| x.get("message").or_else(|| x.get("msg")))
        .and_then(|x| x.as_str())
    {
        return Some(msg.to_string());
    }
    if let Some(msg) = v.get("message").and_then(|x| x.as_str()) {
        let lowered = msg.to_ascii_lowercase();
        if lowered.contains("error") || lowered.contains("invalid") || lowered.contains("denied") {
            return Some(msg.to_string());
        }
    }
    None
}

fn extract_openai_like_content(v: &Value) -> Option<String> {
    let content = v
        .get("choices")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("message"))
        .and_then(|x| x.get("content"))?;
    if let Some(s) = content.as_str() {
        return Some(s.to_string());
    }
    if let Some(arr) = content.as_array() {
        let mut out = String::new();
        for item in arr {
            if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
                out.push_str(s);
            }
        }
        if !out.is_empty() {
            return Some(out);
        }
    }
    None
}

fn extract_anthropic_content(v: &Value) -> Option<String> {
    let arr = v.get("content")?.as_array()?;
    let mut out = String::new();
    for item in arr {
        if let Some(s) = item.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn extract_google_content(v: &Value) -> Option<String> {
    let parts = v
        .get("candidates")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("content"))
        .and_then(|x| x.get("parts"))
        .and_then(|x| x.as_array())?;
    let mut out = String::new();
    for p in parts {
        if let Some(s) = p.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn extract_bedrock_content(v: &Value) -> Option<String> {
    let parts = v
        .get("output")
        .and_then(|x| x.get("message"))
        .and_then(|x| x.get("content"))
        .and_then(|x| x.as_array())?;
    let mut out = String::new();
    for p in parts {
        if let Some(s) = p.get("text").and_then(|x| x.as_str()) {
            out.push_str(s);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Return comma-separated feature flags for a model.
/// Grok :online models get web-search; all Grok models get reasoning.
pub(crate) fn model_features(model: &str) -> String {
    let mut features = Vec::new();
    if model.starts_with("x-ai/grok") || model.starts_with("xai/grok") {
        features.push("reasoning");
        if model.contains(":online") {
            features.push("web-search");
            features.push("x-search");
        }
    }
    features.join(",")
}

fn run_curl_json_post(
    url: &str,
    headers: &[String],
    payload: &Value,
    timeout: TimeoutConfig,
) -> Result<Value, String> {
    let payload_str =
        serde_json::to_string(payload).map_err(|e| format!("json encode failed: {e}"))?;
    let mut cmd = Command::new("curl");
    cmd.arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("--connect-timeout")
        .arg(timeout.connect_timeout_secs.to_string())
        .arg("--max-time")
        .arg(timeout.max_time_secs.to_string())
        .arg(url)
        .arg("-H")
        .arg("Content-Type: application/json");
    for h in headers {
        cmd.arg("-H").arg(h);
    }
    cmd.arg("-d").arg(payload_str);
    let output = cmd.output().map_err(|e| format!("curl exec failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("curl failed: {}", clip(&stderr, 320)));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed = parse_json_response(&stdout)?;
    if let Some(msg) = json_error_message(&parsed) {
        return Err(msg);
    }
    Ok(parsed)
}

fn request_openrouter_with_key(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout("HARMONIA_OPENROUTER", 10, 45);
    let feats = model_features(model);
    let mut payload = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
    });
    if !feats.is_empty() {
        if feats.contains("reasoning") {
            payload["reasoning"] = json!({"effort": "high"});
        }
        if feats.contains("web-search") {
            payload["plugins"] = json!([{ "id": "web-search" }]);
        }
    }
    let headers = vec![
        format!("Authorization: Bearer {api_key}"),
        "HTTP-Referer: https://harmoniis.local".to_string(),
        "X-Title: Harmonia Agent".to_string(),
    ];
    let parsed = run_curl_json_post(OPENROUTER_URL, &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing OpenRouter content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn request_openrouter(prompt: &str, model: &str) -> Result<String, String> {
    let api_key = openrouter_key()?.ok_or_else(|| "openrouter key missing in vault".to_string())?;
    request_openrouter_with_key(prompt, model, &api_key)
}

fn request_openai_compatible(
    provider: Provider,
    prompt: &str,
    model: &str,
    api_key: &str,
) -> Result<String, String> {
    let timeout_prefix = match provider {
        Provider::OpenAI => "HARMONIA_OPENAI",
        Provider::Xai => "HARMONIA_XAI",
        Provider::Groq => "HARMONIA_GROQ",
        Provider::Alibaba => "HARMONIA_ALIBABA",
        _ => "HARMONIA_LLM",
    };
    let timeout = get_timeout(timeout_prefix, 10, 60);
    let url = match provider {
        Provider::OpenAI => {
            env::var("HARMONIA_OPENAI_BASE_URL").unwrap_or_else(|_| OPENAI_URL.to_string())
        }
        Provider::Xai => env::var("HARMONIA_XAI_BASE_URL").unwrap_or_else(|_| XAI_URL.to_string()),
        Provider::Groq => {
            env::var("HARMONIA_GROQ_BASE_URL").unwrap_or_else(|_| GROQ_URL.to_string())
        }
        Provider::Alibaba => {
            env::var("HARMONIA_ALIBABA_BASE_URL").unwrap_or_else(|_| ALIBABA_URL.to_string())
        }
        _ => OPENAI_URL.to_string(),
    };
    let native_model = normalize_model_for_provider(provider, model);
    let payload = json!({
        "model": native_model,
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![format!("Authorization: Bearer {api_key}")];
    let parsed = run_curl_json_post(&url, &headers, &payload, timeout)?;
    extract_openai_like_content(&parsed).ok_or_else(|| {
        format!(
            "missing content in OpenAI-compatible response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn request_anthropic(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout("HARMONIA_ANTHROPIC", 10, 60);
    let api_version =
        env::var("HARMONIA_ANTHROPIC_VERSION").unwrap_or_else(|_| "2023-06-01".to_string());
    let native_model = normalize_model_for_provider(Provider::Anthropic, model);
    let max_tokens = env::var("HARMONIA_ANTHROPIC_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1024);
    let payload = json!({
        "model": native_model,
        "max_tokens": max_tokens,
        "messages": [{"role": "user", "content": prompt}],
    });
    let headers = vec![
        format!("x-api-key: {api_key}"),
        format!("anthropic-version: {api_version}"),
    ];
    let parsed = run_curl_json_post(ANTHROPIC_URL, &headers, &payload, timeout)?;
    extract_anthropic_content(&parsed).ok_or_else(|| {
        format!(
            "missing Anthropic content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn request_google_ai_studio(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let timeout = get_timeout("HARMONIA_GOOGLE_AI_STUDIO", 10, 60);
    let native_model = normalize_model_for_provider(Provider::GoogleAiStudio, model);
    let base = env::var("HARMONIA_GOOGLE_AI_STUDIO_BASE_URL")
        .unwrap_or_else(|_| GOOGLE_AI_STUDIO_BASE.to_string());
    let url = format!(
        "{}/models/{}:generateContent?key={}",
        base.trim_end_matches('/'),
        native_model,
        api_key
    );
    let payload = json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": prompt}]
        }]
    });
    let parsed = run_curl_json_post(&url, &[], &payload, timeout)?;
    extract_google_content(&parsed).ok_or_else(|| {
        format!(
            "missing Google AI Studio content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn vertex_project_location() -> Result<(String, String), String> {
    let project = get_secret_any("google-vertex-backend", &["google-vertex-project-id", "vertex-project-id"])?
        .or_else(|| env::var("HARMONIA_GOOGLE_VERTEX_PROJECT_ID").ok())
        .ok_or_else(|| "google vertex project id missing (vault symbol google-vertex-project-id or env HARMONIA_GOOGLE_VERTEX_PROJECT_ID)".to_string())?;
    let location = get_secret_any(
        "google-vertex-backend",
        &["google-vertex-location", "vertex-location"],
    )?
    .or_else(|| env::var("HARMONIA_GOOGLE_VERTEX_LOCATION").ok())
    .unwrap_or_else(|| "us-central1".to_string());
    Ok((project, location))
}

fn request_google_vertex(prompt: &str, model: &str, token: &str) -> Result<String, String> {
    let timeout = get_timeout("HARMONIA_GOOGLE_VERTEX", 10, 60);
    let (project, location) = vertex_project_location()?;
    let endpoint = env::var("HARMONIA_GOOGLE_VERTEX_ENDPOINT")
        .unwrap_or_else(|_| format!("https://{}-aiplatform.googleapis.com", location));
    let native_model = normalize_model_for_provider(Provider::GoogleVertex, model);
    let url = format!(
        "{}/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
        endpoint.trim_end_matches('/'),
        project,
        location,
        native_model
    );
    let payload = json!({
        "contents": [{
            "role": "user",
            "parts": [{"text": prompt}]
        }]
    });
    let headers = vec![format!("Authorization: Bearer {token}")];
    let parsed = run_curl_json_post(&url, &headers, &payload, timeout)?;
    extract_google_content(&parsed).ok_or_else(|| {
        format!(
            "missing Google Vertex content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn apply_aws_env(cmd: &mut Command) -> Result<(), String> {
    let map = [
        ("aws-access-key-id", "AWS_ACCESS_KEY_ID"),
        ("aws-secret-access-key", "AWS_SECRET_ACCESS_KEY"),
        ("aws-session-token", "AWS_SESSION_TOKEN"),
        ("aws-region", "AWS_REGION"),
    ];
    for (symbol, env_name) in map {
        if let Some(v) = get_secret_for_component("amazon-bedrock-backend", symbol)
            .map_err(|e| format!("vault policy error: {e}"))?
        {
            if !v.trim().is_empty() {
                cmd.env(env_name, v);
            }
        }
    }
    Ok(())
}

fn request_bedrock(prompt: &str, model: &str) -> Result<String, String> {
    let region = get_secret_any("amazon-bedrock-backend", &["aws-region"])?
        .or_else(|| env::var("AWS_REGION").ok())
        .or_else(|| env::var("HARMONIA_BEDROCK_REGION").ok())
        .unwrap_or_else(|| "us-east-1".to_string());
    let native_model = normalize_model_for_provider(Provider::AmazonBedrock, model);
    let messages = json!([
        {
            "role": "user",
            "content": [{"text": prompt}]
        }
    ]);
    let mut cmd = Command::new("aws");
    cmd.arg("bedrock-runtime")
        .arg("converse")
        .arg("--region")
        .arg(region)
        .arg("--model-id")
        .arg(native_model)
        .arg("--messages")
        .arg(messages.to_string())
        .arg("--output")
        .arg("json");
    apply_aws_env(&mut cmd)?;
    let output = cmd
        .output()
        .map_err(|e| format!("aws cli exec failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "aws bedrock-runtime converse failed: {}",
            clip(&String::from_utf8_lossy(&output.stderr), 320)
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let parsed = parse_json_response(&stdout)?;
    if let Some(msg) = json_error_message(&parsed) {
        return Err(msg);
    }
    extract_bedrock_content(&parsed).ok_or_else(|| {
        format!(
            "missing Bedrock content in response: {}",
            clip(&parsed.to_string(), 320)
        )
    })
}

fn request_native_or_fallback(
    prompt: &str,
    model: &str,
    provider: Provider,
) -> Result<String, String> {
    fn fallback_openrouter(prompt: &str, model: &str, err: String) -> Result<String, String> {
        if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
            return request_openrouter(prompt, model);
        }
        Err(err)
    }

    let native_key = provider_key(provider)?;
    match provider {
        Provider::OpenRouter => request_openrouter(prompt, model),
        Provider::OpenAI => {
            if let Some(key) = native_key {
                match request_openai_compatible(provider, prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("openai api key missing in vault".to_string())
            }
        }
        Provider::Xai => {
            if let Some(key) = native_key {
                match request_openai_compatible(provider, prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("xai api key missing in vault".to_string())
            }
        }
        Provider::Groq => {
            if let Some(key) = native_key {
                match request_openai_compatible(provider, prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("groq api key missing in vault".to_string())
            }
        }
        Provider::Alibaba => {
            if let Some(key) = native_key {
                match request_openai_compatible(provider, prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("alibaba api key missing in vault".to_string())
            }
        }
        Provider::Anthropic => {
            if let Some(key) = native_key {
                match request_anthropic(prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("anthropic api key missing in vault".to_string())
            }
        }
        Provider::GoogleAiStudio => {
            if let Some(key) = native_key {
                match request_google_ai_studio(prompt, model, &key) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("google ai studio api key missing in vault".to_string())
            }
        }
        Provider::GoogleVertex => {
            if let Some(token) = native_key {
                match request_google_vertex(prompt, model, &token) {
                    Ok(v) => Ok(v),
                    Err(e) => fallback_openrouter(prompt, model, e),
                }
            } else if openrouter_fallback_enabled() && openrouter_key()?.is_some() {
                request_openrouter(prompt, model)
            } else {
                Err("google vertex access token missing in vault".to_string())
            }
        }
        Provider::AmazonBedrock => match request_bedrock(prompt, model) {
            Ok(v) => Ok(v),
            Err(e) => fallback_openrouter(prompt, model, e),
        },
    }
}

fn request_model(prompt: &str, model: &str) -> Result<String, String> {
    let provider = provider_from_model(model);
    request_native_or_fallback(prompt, model, provider)
}

pub(crate) fn init_backend() -> Result<(), String> {
    init_from_env()
}

pub(crate) fn complete(prompt: &str, model: &str) -> Result<String, String> {
    let _ = init_from_env();
    let selected_model = if model.trim().is_empty() {
        default_model()?
    } else {
        model.to_string()
    };

    match request_model(prompt, &selected_model) {
        Ok(text) => Ok(text),
        Err(primary_err) => {
            for fallback in fallback_models() {
                if fallback == selected_model {
                    continue;
                }
                if let Ok(text) = request_model(prompt, &fallback) {
                    return Ok(text);
                }
            }
            Err(primary_err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_anthropic_content, extract_google_content, extract_openai_like_content};
    use serde_json::json;

    #[test]
    fn extract_openai_style_text() {
        let v = json!({"choices":[{"message":{"content":"hello"}}]});
        assert_eq!(extract_openai_like_content(&v).as_deref(), Some("hello"));
    }

    #[test]
    fn extract_anthropic_style_text() {
        let v = json!({"content":[{"type":"text","text":"alpha"},{"type":"text","text":"beta"}]});
        assert_eq!(extract_anthropic_content(&v).as_deref(), Some("alphabeta"));
    }

    #[test]
    fn extract_google_style_text() {
        let v = json!({"candidates":[{"content":{"parts":[{"text":"ok"}]}}]});
        assert_eq!(extract_google_content(&v).as_deref(), Some("ok"));
    }
}
