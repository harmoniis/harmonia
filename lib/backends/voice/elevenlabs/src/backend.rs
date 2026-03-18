use harmonia_voice_protocol::*;
use serde_json::json;
use std::io::Read;

const COMPONENT: &str = "elevenlabs-backend";
const BASE_URL: &str = "https://api.elevenlabs.io/v1";

pub static OFFERINGS: &[VoiceOffering] = &[
    VoiceOffering {
        id: "elevenlabs/eleven_multilingual_v2",
        provider: "elevenlabs",
        kind: VoiceKind::TextToSpeech,
        quality: 9,
        speed: 7,
        tags: &["tts", "multilingual", "expressive"],
    },
    VoiceOffering {
        id: "elevenlabs/eleven_turbo_v2_5",
        provider: "elevenlabs",
        kind: VoiceKind::TextToSpeech,
        quality: 7,
        speed: 9,
        tags: &["tts", "fast", "low-latency"],
    },
    VoiceOffering {
        id: "elevenlabs/eleven_monolingual_v1",
        provider: "elevenlabs",
        kind: VoiceKind::TextToSpeech,
        quality: 8,
        speed: 8,
        tags: &["tts", "english"],
    },
];

fn api_key() -> Result<String, String> {
    get_secret_any(COMPONENT, &["elevenlabs-api-key", "elevenlabs"])?
        .ok_or_else(|| "elevenlabs api key missing in vault".to_string())
}

fn base_url() -> String {
    harmonia_config_store::get_own_or(COMPONENT, "base-url", BASE_URL)
        .unwrap_or_else(|_| BASE_URL.to_string())
}

pub fn init() -> Result<(), String> {
    harmonia_voice_protocol::harmonia_vault::init_from_env().ok();
    Ok(())
}

pub fn tts_to_file(text: &str, voice_id: &str, out_path: &str, model: &str) -> Result<(), String> {
    let _ = init();
    let key = api_key()?;
    let timeout = get_timeout(COMPONENT, "HARMONIA_ELEVENLABS", 10, 60);

    let model_id = if model.is_empty() {
        "eleven_multilingual_v2"
    } else {
        strip_provider_prefix(model)
    };

    let url = format!("{}/text-to-speech/{}", base_url(), voice_id);
    let payload = json!({
        "text": text,
        "model_id": model_id,
    });

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(timeout.connect_secs))
        .timeout(std::time::Duration::from_secs(timeout.max_secs))
        .build();

    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .set("xi-api-key", &key)
        .send_string(&payload.to_string())
        .map_err(|e| format!("elevenlabs request failed: {e}"))?;

    let mut audio_data = Vec::new();
    resp.into_reader()
        .take(50 * 1024 * 1024) // 50MB max for audio
        .read_to_end(&mut audio_data)
        .map_err(|e| format!("failed to read audio data: {e}"))?;

    std::fs::write(out_path, &audio_data)
        .map_err(|e| format!("failed to write audio file {out_path}: {e}"))?;

    Ok(())
}

pub fn list_voices() -> Result<String, String> {
    let _ = init();
    let key = api_key()?;
    let timeout = get_timeout(COMPONENT, "HARMONIA_ELEVENLABS", 10, 30);

    let url = format!("{}/voices", base_url());

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(timeout.connect_secs))
        .timeout(std::time::Duration::from_secs(timeout.max_secs))
        .build();

    let resp = agent
        .get(&url)
        .set("xi-api-key", &key)
        .call()
        .map_err(|e| format!("elevenlabs list voices failed: {e}"))?;

    let mut body = String::new();
    resp.into_reader()
        .take(4 * 1024 * 1024)
        .read_to_string(&mut body)
        .map_err(|e| format!("failed to read response: {e}"))?;

    Ok(body)
}

pub fn list_offerings() -> String {
    offerings_to_sexp(OFFERINGS)
}
