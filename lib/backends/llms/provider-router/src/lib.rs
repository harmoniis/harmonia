//! Harmonia Provider Router — multi-backend dispatch with vault-based activation.
//!
//! Routes LLM requests to native provider backends when the user has configured
//! an API key in vault during `harmonia setup`. Falls back to OpenRouter as the
//! universal gateway when no native key is available for the requested model's
//! provider.
//!
//! Dispatch logic:
//! 1. Extract provider prefix from model ID (e.g. "anthropic/" from "anthropic/claude-sonnet-4.6")
//! 2. Check if the native backend for that provider has a vault key
//! 3. If yes → route to native backend
//! 4. If no → route to OpenRouter (which handles all providers via its gateway)

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

use harmonia_provider_protocol::{clear_error, last_error_message, set_error};

use harmonia_alibaba::backend as alibaba;
use harmonia_amazon_bedrock::backend as bedrock;
use harmonia_anthropic::backend as anthropic;
use harmonia_google_ai_studio::backend as google_ai_studio;
use harmonia_google_vertex::backend as google_vertex;
use harmonia_groq::backend as groq;
use harmonia_openai::backend as openai;
use harmonia_openrouter::client as openrouter;
use harmonia_xai::backend as xai;

const VERSION: &[u8] = b"harmonia-provider-router/0.2.0\0";

// ── Provider registry ────────────────────────────────────────────────

/// A native backend that can be activated when its vault key is present.
struct ProviderEntry {
    /// Provider ID used in backend status reporting.
    id: &'static str,
    /// Model ID prefixes that route to this backend (e.g. "anthropic/").
    prefixes: &'static [&'static str],
    /// Vault component and secret symbols to check for activation.
    vault_component: &'static str,
    vault_symbols: &'static [&'static str],
}

static PROVIDERS: &[ProviderEntry] = &[
    ProviderEntry {
        id: "anthropic",
        prefixes: &["anthropic/"],
        vault_component: "anthropic-backend",
        vault_symbols: &["anthropic-api-key", "anthropic"],
    },
    ProviderEntry {
        id: "openai",
        prefixes: &["openai/"],
        vault_component: "openai-backend",
        vault_symbols: &["openai-api-key", "openai"],
    },
    ProviderEntry {
        id: "xai",
        prefixes: &["x-ai/", "xai/"],
        vault_component: "xai-backend",
        vault_symbols: &["xai-api-key", "x-ai-api-key", "xai"],
    },
    ProviderEntry {
        id: "google-ai-studio",
        prefixes: &["google/"],
        vault_component: "google-ai-studio-backend",
        vault_symbols: &[
            "google-ai-studio-api-key",
            "gemini-api-key",
            "google-api-key",
        ],
    },
    ProviderEntry {
        id: "google-vertex",
        prefixes: &["vertex/"],
        vault_component: "google-vertex-backend",
        vault_symbols: &["google-vertex-access-token", "vertex-access-token"],
    },
    ProviderEntry {
        id: "bedrock",
        prefixes: &["amazon/"],
        vault_component: "bedrock-backend",
        vault_symbols: &["aws-access-key-id"],
    },
    ProviderEntry {
        id: "groq",
        prefixes: &["groq/"],
        vault_component: "groq-backend",
        vault_symbols: &["groq-api-key", "groq"],
    },
    ProviderEntry {
        id: "alibaba",
        prefixes: &["qwen/", "alibaba/", "dashscope/"],
        vault_component: "alibaba-backend",
        vault_symbols: &["alibaba-api-key", "dashscope-api-key", "alibaba"],
    },
];

// ── Vault key detection (cached) ─────────────────────────────────────

/// Cached set of provider IDs that have vault keys available.
static ACTIVE_PROVIDERS: OnceLock<Vec<&'static str>> = OnceLock::new();

fn detect_active_providers() -> Vec<&'static str> {
    let mut active = Vec::new();
    for p in PROVIDERS {
        if has_vault_key(p.vault_component, p.vault_symbols) {
            active.push(p.id);
        }
    }
    active
}

fn active_providers() -> &'static Vec<&'static str> {
    ACTIVE_PROVIDERS.get_or_init(detect_active_providers)
}

fn has_vault_key(component: &str, symbols: &[&str]) -> bool {
    harmonia_provider_protocol::get_secret_any(component, symbols)
        .ok()
        .flatten()
        .is_some()
}

fn provider_is_active(provider_id: &str) -> bool {
    active_providers().contains(&provider_id)
}

// ── Dispatch logic ───────────────────────────────────────────────────

/// Find the provider entry matching a model ID prefix.
fn resolve_provider(model: &str) -> Option<&'static ProviderEntry> {
    let lower = model.to_ascii_lowercase();
    PROVIDERS
        .iter()
        .find(|p| p.prefixes.iter().any(|prefix| lower.starts_with(prefix)))
}

/// Route a completion to the appropriate native backend.
/// Returns Err if the backend fails (caller should fallback to OpenRouter).
fn dispatch_native(provider_id: &str, prompt: &str, model: &str) -> Result<String, String> {
    match provider_id {
        "anthropic" => anthropic::complete(prompt, model),
        "openai" => openai::complete(prompt, model),
        "xai" => xai::complete(prompt, model),
        "google-ai-studio" => google_ai_studio::complete(prompt, model),
        "google-vertex" => google_vertex::complete(prompt, model),
        "bedrock" => bedrock::complete(prompt, model),
        "groq" => groq::complete(prompt, model),
        "alibaba" => alibaba::complete(prompt, model),
        _ => Err(format!("unknown native provider: {provider_id}")),
    }
}

#[allow(dead_code)]
fn dispatch_native_for_task(
    provider_id: &str,
    prompt: &str,
    task_hint: &str,
) -> Result<String, String> {
    match provider_id {
        "anthropic" => anthropic::complete_for_task(prompt, task_hint),
        "openai" => openai::complete_for_task(prompt, task_hint),
        "xai" => xai::complete_for_task(prompt, task_hint),
        "google-ai-studio" => google_ai_studio::complete_for_task(prompt, task_hint),
        "google-vertex" => google_vertex::complete_for_task(prompt, task_hint),
        "bedrock" => bedrock::complete_for_task(prompt, task_hint),
        "groq" => groq::complete_for_task(prompt, task_hint),
        "alibaba" => alibaba::complete_for_task(prompt, task_hint),
        _ => Err(format!("unknown native provider: {provider_id}")),
    }
}

/// Main routing: try native backend first, fallback to OpenRouter.
fn route_complete(prompt: &str, model: &str) -> Result<String, String> {
    if let Some(provider) = resolve_provider(model) {
        if provider_is_active(provider.id) {
            match dispatch_native(provider.id, prompt, model) {
                Ok(text) => return Ok(text),
                Err(_native_err) => {
                    // Native backend failed — fallback to OpenRouter
                }
            }
        }
    }
    // Fallback: OpenRouter handles all model prefixes
    openrouter::complete(prompt, model)
}

fn route_complete_for_task(prompt: &str, task_hint: &str) -> Result<String, String> {
    // Task-based routing doesn't have a model prefix to resolve,
    // so we route through OpenRouter which has the full model pool.
    openrouter::complete_for_task(prompt, task_hint)
}

// ── Backend status ───────────────────────────────────────────────────

fn all_backends_sexp() -> String {
    let mut entries = Vec::new();
    // Always include OpenRouter as the universal gateway
    entries.push(
        "(:id \"openrouter\" :healthy t :default t :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))".to_string()
    );
    // Add active native providers
    for p in PROVIDERS {
        if provider_is_active(p.id) {
            entries.push(format!(
                "(:id \"{}\" :healthy t :default nil :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))",
                p.id
            ));
        }
    }
    format!("({})", entries.join(" "))
}

fn backend_status_sexp(name: &str) -> Option<String> {
    if name.is_empty() || name == "openrouter" {
        return Some(
            "(:id \"openrouter\" :healthy t :default t :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))".to_string()
        );
    }
    let provider = PROVIDERS.iter().find(|p| p.id == name)?;
    let active = provider_is_active(provider.id);
    Some(format!(
        "(:id \"{}\" :healthy {} :default nil :capabilities (\"complete\" \"complete-for-task\" \"list-models\" \"select-model\"))",
        provider.id,
        if active { "t" } else { "nil" }
    ))
}

// ── Init ─────────────────────────────────────────────────────────────

fn init_all() -> Result<(), String> {
    // Always init OpenRouter (universal fallback)
    openrouter::init_backend()?;
    // Init native backends that have vault keys
    // Errors are non-fatal — if a native backend fails to init,
    // requests will fallback to OpenRouter
    let active = active_providers();
    for id in active.iter() {
        let _ = match *id {
            "anthropic" => anthropic::init(),
            "openai" => openai::init(),
            "xai" => xai::init(),
            "google-ai-studio" => google_ai_studio::init(),
            "google-vertex" => google_vertex::init(),
            "bedrock" => bedrock::init(),
            "groq" => groq::init(),
            "alibaba" => alibaba::init(),
            _ => Ok(()),
        };
    }
    Ok(())
}

// ── FFI exports ──────────────────────────────────────────────────────

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_init() -> i32 {
    match init_all() {
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
pub extern "C" fn harmonia_provider_router_complete(
    prompt: *const c_char,
    model: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let model = cstr_to_string(model).unwrap_or_default();
    match route_complete(&prompt, &model) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_complete_for_task(
    prompt: *const c_char,
    task_hint: *const c_char,
) -> *mut c_char {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let task_hint = cstr_to_string(task_hint).unwrap_or_default();
    match route_complete_for_task(&prompt, &task_hint) {
        Ok(text) => {
            clear_error();
            to_c_string(text)
        }
        Err(e) => {
            set_error(e);
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_list_models() -> *mut c_char {
    clear_error();
    to_c_string(openrouter::list_offerings())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_select_model(task_hint: *const c_char) -> *mut c_char {
    let task_hint = cstr_to_string(task_hint).unwrap_or_default();
    clear_error();
    to_c_string(openrouter::select_model_for_task(&task_hint))
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_list_backends() -> *mut c_char {
    clear_error();
    to_c_string(all_backends_sexp())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_backend_status(name: *const c_char) -> *mut c_char {
    let name = cstr_to_string(name).unwrap_or_default();
    match backend_status_sexp(&name) {
        Some(sexp) => {
            clear_error();
            to_c_string(sexp)
        }
        None => {
            set_error(format!("unknown backend adapter: {name}"));
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_provider_router_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
