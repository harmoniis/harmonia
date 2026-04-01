//! Initialization — boot all active native backends.

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

use crate::registry::active_providers;

pub fn init_all() -> Result<(), String> {
    // Always init OpenRouter (universal fallback)
    openrouter::init_backend()?;
    // Init native backends that have vault keys.
    // Errors are non-fatal — if a native backend fails to init,
    // requests will fallback to OpenRouter.
    for id in active_providers().iter() {
        let _ = match *id {
            "anthropic" => anthropic::init(),
            "openai" => openai::init(),
            "xai" => xai::init(),
            "google-ai-studio" => google_ai_studio::init(),
            "google-vertex" => google_vertex::init(),
            "bedrock" => bedrock::init(),
            "groq" => groq::init(),
            "alibaba" => alibaba::init(),
            "harmoniis" => harmoniis::init(),
            _ => Ok(()),
        };
    }
    Ok(())
}
