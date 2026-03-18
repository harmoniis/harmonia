//! Harmonia Voice Router
//!
//! Multi-backend dispatch for speech-to-text and text-to-speech operations.
//! Routes to the appropriate voice provider based on model prefix and vault
//! activation, with automatic fallback chains.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

use harmonia_voice_protocol::{clear_error, get_secret_any, last_error_message, set_error};

// ── Provider Registry ──────────────────────────────────────────────────────

struct VoiceProvider {
    id: &'static str,
    #[allow(dead_code)] // Used for future model-prefix routing
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

// ── Active Provider Detection ──────────────────────────────────────────────

static ACTIVE_PROVIDERS: OnceLock<Vec<String>> = OnceLock::new();

fn active_providers() -> &'static Vec<String> {
    ACTIVE_PROVIDERS.get_or_init(|| {
        let mut active = Vec::new();
        for p in PROVIDERS {
            if get_secret_any(p.vault_component, p.vault_symbols)
                .ok()
                .flatten()
                .is_some()
            {
                active.push(p.id.to_string());
            }
        }
        active
    })
}

fn is_provider_active(id: &str) -> bool {
    active_providers().iter().any(|a| a == id)
}

// ── Routing ────────────────────────────────────────────────────────────────

pub fn transcribe(audio_path: &str, model_hint: &str) -> Result<String, String> {
    harmonia_whisper::backend::transcribe(audio_path, model_hint)
}

pub fn tts_to_file(
    text: &str,
    voice_id: &str,
    out_path: &str,
    model_hint: &str,
) -> Result<(), String> {
    harmonia_elevenlabs::backend::tts_to_file(text, voice_id, out_path, model_hint)
}

pub fn list_providers() -> String {
    let mut parts = Vec::new();
    for p in PROVIDERS {
        let active = is_provider_active(p.id);
        parts.push(format!(
            "(:id \"{}\" :active {})",
            p.id,
            if active { "t" } else { "nil" }
        ));
    }
    format!("({})", parts.join(" "))
}

pub fn init() -> Result<(), String> {
    harmonia_whisper::backend::init()?;
    harmonia_elevenlabs::backend::init()?;
    let _ = active_providers();
    Ok(())
}

// ── FFI Exports ────────────────────────────────────────────────────────────

const VERSION: &[u8] = b"harmonia-voice-router/0.1.0\0";

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    Ok(unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned())
}

fn to_c(value: String) -> *mut c_char {
    CString::new(value)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_init() -> i32 {
    match init() {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_transcribe(
    audio_path: *const c_char,
    model_hint: *const c_char,
) -> *mut c_char {
    let path = match cstr_to_string(audio_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let hint = cstr_to_string(model_hint).unwrap_or_default();
    match transcribe(&path, &hint) {
        Ok(text) => {
            clear_error();
            to_c(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_tts(
    text: *const c_char,
    voice_id: *const c_char,
    out_path: *const c_char,
    model_hint: *const c_char,
) -> i32 {
    let text = match cstr_to_string(text) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let voice = match cstr_to_string(voice_id) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let out = match cstr_to_string(out_path) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let hint = cstr_to_string(model_hint).unwrap_or_default();
    match tts_to_file(&text, &voice, &out, &hint) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_list_providers() -> *mut c_char {
    to_c(list_providers())
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_last_error() -> *mut c_char {
    to_c(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_voice_router_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr));
        }
    }
}
