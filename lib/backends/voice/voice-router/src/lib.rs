//! Harmonia Voice Router — speech-to-text + text-to-speech dispatch as a ractor component.
//!
//! Reached ONLY through the s-expr IPC actor boundary (NO FFI), exactly like every other
//! component. The Lisp side owns ROUTING POLICY (which endpoint/tier — see voice-routing.lisp
//! + voice-policy.sexp); this actor owns EXECUTION: given a resolved `model` hint it calls the
//! right backend (Whisper STT, ElevenLabs TTS, or a CUSTOM OpenAI-compatible endpoint defined
//! in config) and returns the result.
//!
//! Ops (`(:component "voice" :op …)`):
//!   transcribe (:audio path :model id)              → (:ok :text "…")
//!   synthesize (:text … :voice id :out path :model) → (:ok :path "…")
//!   offerings                                        → (:ok :stt (…) :tts (…))
//!   providers                                        → (:ok :providers (…))
//!   ready                                            → (:ok :ready t)
//!
//! Separation of concerns: the actual HTTP call goes through the voice-protocol transport
//! helpers (the swappable seam) — a streaming transport lands behind the same boundary in the
//! streaming phase without touching this router.

use std::sync::OnceLock;

use harmonia_actor_protocol::extract_sexp_string;
use harmonia_voice_protocol::{
    get_secret_any, get_timeout, strip_provider_prefix, ureq_post_json_bytes, ureq_post_multipart,
};

// ── Provider registry (vault-activated, like the LLM provider-router) ────────

struct VoiceProvider {
    id: &'static str,
    prefixes: &'static [&'static str],
    vault_component: &'static str,
    vault_symbols: &'static [&'static str],
}

static PROVIDERS: &[VoiceProvider] = &[
    VoiceProvider {
        id: "whisper-groq",
        prefixes: &["groq/whisper"],
        vault_component: "whisper-backend",
        vault_symbols: &["groq-api-key", "groq"],
    },
    VoiceProvider {
        id: "whisper-openai",
        prefixes: &["openai/whisper"],
        vault_component: "whisper-backend",
        vault_symbols: &["openai-api-key", "openai"],
    },
    VoiceProvider {
        id: "elevenlabs",
        prefixes: &["elevenlabs/"],
        vault_component: "elevenlabs-backend",
        vault_symbols: &["elevenlabs-api-key", "elevenlabs"],
    },
];

static ACTIVE_PROVIDERS: OnceLock<Vec<String>> = OnceLock::new();

fn active_providers() -> &'static Vec<String> {
    ACTIVE_PROVIDERS.get_or_init(|| {
        PROVIDERS
            .iter()
            .filter(|p| {
                get_secret_any(p.vault_component, p.vault_symbols)
                    .ok()
                    .flatten()
                    .is_some()
            })
            .map(|p| p.id.to_string())
            .collect()
    })
}

fn is_provider_active(id: &str) -> bool {
    active_providers().iter().any(|a| a == id)
}

fn config_url(key: &str) -> Option<String> {
    harmonia_config_store::get_own("voice", key)
        .ok()
        .flatten()
        .filter(|u| !u.is_empty())
}

fn is_custom(model_hint: &str) -> bool {
    model_hint.to_ascii_lowercase().starts_with("custom/")
}

// ── Execution (the transport seam — batch now, streaming later) ──────────────

pub fn transcribe(audio_path: &str, model_hint: &str) -> Result<String, String> {
    if is_custom(model_hint) {
        return custom_transcribe(audio_path, model_hint);
    }
    harmonia_whisper::backend::transcribe(audio_path, model_hint)
}

pub fn synthesize(
    text: &str,
    voice_id: &str,
    out_path: &str,
    model_hint: &str,
) -> Result<(), String> {
    if is_custom(model_hint) {
        return custom_synthesize(text, voice_id, out_path, model_hint);
    }
    harmonia_elevenlabs::backend::tts_to_file(text, voice_id, out_path, model_hint)
}

/// Custom OpenAI-compatible STT endpoint: config `voice/custom-stt-url`, vault `custom-stt-backend`.
fn custom_transcribe(audio_path: &str, model: &str) -> Result<String, String> {
    let url = config_url("custom-stt-url")
        .ok_or_else(|| "custom STT endpoint not configured (config voice/custom-stt-url)".to_string())?;
    let key = get_secret_any("custom-stt-backend", &["custom-stt-api-key", "custom-stt"])?
        .ok_or_else(|| "custom STT key missing (vault custom-stt-backend)".to_string())?;
    let native_model =
        harmonia_config_store::get_own_or("voice", "custom-stt-model", strip_provider_prefix(model))
            .unwrap_or_else(|_| strip_provider_prefix(model).to_string());
    let timeout = get_timeout("custom-stt-backend", "HARMONIA_CUSTOM_STT", 10, 120);
    let raw = ureq_post_multipart(&url, &key, &[("model", &native_model)], "file", audio_path, &timeout)?;
    let v: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("custom STT: invalid JSON: {e}"))?;
    Ok(v.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string())
}

/// Custom OpenAI-compatible TTS endpoint: config `voice/custom-tts-url`, vault `custom-tts-backend`.
fn custom_synthesize(text: &str, voice_id: &str, out_path: &str, model: &str) -> Result<(), String> {
    let url = config_url("custom-tts-url")
        .ok_or_else(|| "custom TTS endpoint not configured (config voice/custom-tts-url)".to_string())?;
    let key = get_secret_any("custom-tts-backend", &["custom-tts-api-key", "custom-tts"])?
        .ok_or_else(|| "custom TTS key missing (vault custom-tts-backend)".to_string())?;
    let native_model =
        harmonia_config_store::get_own_or("voice", "custom-tts-model", strip_provider_prefix(model))
            .unwrap_or_else(|_| strip_provider_prefix(model).to_string());
    let timeout = get_timeout("custom-tts-backend", "HARMONIA_CUSTOM_TTS", 10, 60);
    let body = serde_json::json!({ "model": native_model, "input": text, "voice": voice_id });
    let headers = vec![("Authorization".to_string(), format!("Bearer {key}"))];
    let audio = ureq_post_json_bytes(&url, &headers, &body, &timeout, 50 * 1024 * 1024)?;
    std::fs::write(out_path, &audio).map_err(|e| format!("custom TTS: write {out_path}: {e}"))
}

// ── Introspection ────────────────────────────────────────────────────────────

fn bool_sexp(b: bool) -> &'static str {
    if b {
        "t"
    } else {
        "nil"
    }
}

pub fn list_providers() -> String {
    let mut parts: Vec<String> = PROVIDERS
        .iter()
        .map(|p| format!("(:id \"{}\" :active {})", p.id, bool_sexp(is_provider_active(p.id))))
        .collect();
    parts.push(format!(
        "(:id \"custom-stt\" :active {})",
        bool_sexp(config_url("custom-stt-url").is_some())
    ));
    parts.push(format!(
        "(:id \"custom-tts\" :active {})",
        bool_sexp(config_url("custom-tts-url").is_some())
    ));
    format!("({})", parts.join(" "))
}

fn offerings_sexp() -> String {
    format!(
        "(:ok :stt {} :tts {})",
        harmonia_whisper::backend::list_offerings(),
        harmonia_elevenlabs::backend::list_offerings()
    )
}

pub fn init() -> Result<(), String> {
    harmonia_whisper::backend::init()?;
    harmonia_elevenlabs::backend::init()?;
    let _ = active_providers();
    Ok(())
}

// ── Actor-owned state + s-expr dispatch ──────────────────────────────────────

#[derive(Default)]
pub struct VoiceState;

impl VoiceState {
    pub fn init() -> Self {
        let _ = init();
        eprintln!(
            "[INFO] [voice] router started (active: {})",
            active_providers().join(",")
        );
        VoiceState
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            '\t' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

/// IPC dispatch — the ONLY entry point from Lisp (via the actor boundary).
pub fn dispatch(_state: &mut VoiceState, sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "transcribe" => {
            let audio = extract_sexp_string(sexp, ":audio").unwrap_or_default();
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            if audio.is_empty() {
                return "(:error \"voice transcribe: :audio required\")".to_string();
            }
            match transcribe(&audio, &model) {
                Ok(text) => format!("(:ok :text \"{}\")", esc(&text)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "synthesize" => {
            let text = extract_sexp_string(sexp, ":text").unwrap_or_default();
            let voice = extract_sexp_string(sexp, ":voice").unwrap_or_default();
            let out = extract_sexp_string(sexp, ":out").unwrap_or_default();
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            if text.is_empty() || out.is_empty() {
                return "(:error \"voice synthesize: :text and :out required\")".to_string();
            }
            match synthesize(&text, &voice, &out, &model) {
                Ok(()) => format!("(:ok :path \"{}\")", esc(&out)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "offerings" => offerings_sexp(),
        "providers" => format!("(:ok :providers {})", list_providers()),
        "ready" => "(:ok :ready t)".to_string(),
        other => format!("(:error \"voice: unknown op '{}'\")", esc(other)),
    }
}
