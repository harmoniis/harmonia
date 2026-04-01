//! Dispatch logic — routes requests to native backends or OpenRouter fallback.

use harmonia_alibaba::backend as alibaba;
use harmonia_amazon_bedrock::backend as bedrock;
use harmonia_anthropic::backend as anthropic;
use harmonia_google_ai_studio::backend as google_ai_studio;
use harmonia_google_vertex::backend as google_vertex;
use harmonia_groq::backend as groq;
use harmonia_harmoniis::backend as harmoniis;
use harmonia_openai::backend as openai;
use harmonia_openrouter::client as openrouter;
use harmonia_xai::backend as xai;

use crate::registry::{provider_is_active, resolve_provider};

/// Route a completion to the appropriate native backend.
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
        "harmoniis" => harmoniis::complete(prompt, model),
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
        "harmoniis" => harmoniis::complete_for_task(prompt, task_hint),
        _ => Err(format!("unknown native provider: {provider_id}")),
    }
}

/// Main routing: try native backend first, fallback to OpenRouter.
pub fn route_complete(prompt: &str, model: &str) -> Result<String, String> {
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
    openrouter::complete(prompt, model)
}

pub fn route_complete_for_task(prompt: &str, task_hint: &str) -> Result<String, String> {
    openrouter::complete_for_task(prompt, task_hint)
}
