use harmonia_voice_protocol::*;

const COMPONENT: &str = "whisper-backend";
const GROQ_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";
const OPENAI_URL: &str = "https://api.openai.com/v1/audio/transcriptions";

pub static OFFERINGS: &[VoiceOffering] = &[
    VoiceOffering {
        id: "groq/whisper-large-v3-turbo",
        provider: "groq",
        kind: VoiceKind::SpeechToText,
        quality: 8,
        speed: 9,
        tags: &["fast", "stt", "multilingual"],
    },
    VoiceOffering {
        id: "groq/whisper-large-v3",
        provider: "groq",
        kind: VoiceKind::SpeechToText,
        quality: 9,
        speed: 7,
        tags: &["stt", "multilingual", "accurate"],
    },
    VoiceOffering {
        id: "openai/whisper-1",
        provider: "openai",
        kind: VoiceKind::SpeechToText,
        quality: 7,
        speed: 6,
        tags: &["stt", "multilingual"],
    },
];

fn groq_api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["groq-api-key", "groq"])
}

fn openai_api_key() -> Result<Option<String>, String> {
    get_secret_any(COMPONENT, &["openai-api-key", "openai"])
}

fn api_url_for_provider(provider: &str) -> String {
    match provider {
        "groq" => harmonia_config_store::get_own_or(COMPONENT, "groq-api-url", GROQ_URL)
            .unwrap_or_else(|_| GROQ_URL.to_string()),
        _ => harmonia_config_store::get_own_or(COMPONENT, "openai-api-url", OPENAI_URL)
            .unwrap_or_else(|_| OPENAI_URL.to_string()),
    }
}

fn api_key_for_provider(provider: &str) -> Result<String, String> {
    match provider {
        "groq" => groq_api_key()?.ok_or_else(|| "groq api key missing in vault".to_string()),
        _ => openai_api_key()?.ok_or_else(|| "openai api key missing in vault".to_string()),
    }
}

fn transcribe_with_provider(
    audio_path: &str,
    model: &str,
    provider: &str,
) -> Result<String, String> {
    let key = api_key_for_provider(provider)?;
    let url = api_url_for_provider(provider);
    let native_model = strip_provider_prefix(model);
    let timeout = get_timeout(COMPONENT, "HARMONIA_WHISPER", 10, 120);

    let raw = ureq_post_multipart(
        &url,
        &key,
        &[("model", native_model)],
        "file",
        audio_path,
        &timeout,
    )?;

    let parsed: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("invalid JSON: {e}"))?;

    parsed
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no 'text' field in response: {}", clip(&raw, 320)))
}

pub fn init() -> Result<(), String> {
    harmonia_voice_protocol::harmonia_vault::init_from_env().ok();
    Ok(())
}

pub fn transcribe(audio_path: &str, model: &str) -> Result<String, String> {
    let _ = init();

    let selected = if model.trim().is_empty() {
        select_from_pool(OFFERINGS, VoiceKind::SpeechToText)
    } else {
        model.to_string()
    };

    let provider = selected.split('/').next().unwrap_or("groq");

    match transcribe_with_provider(audio_path, &selected, provider) {
        Ok(text) => Ok(text),
        Err(primary_err) => {
            for fb in pool_fallbacks(OFFERINGS, &selected) {
                let fb_provider = fb.split('/').next().unwrap_or("openai");
                if let Ok(text) = transcribe_with_provider(audio_path, &fb, fb_provider) {
                    return Ok(text);
                }
            }
            Err(primary_err)
        }
    }
}

pub fn list_offerings() -> String {
    offerings_to_sexp(OFFERINGS)
}
