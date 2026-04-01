//! Provider registry — static table of native backends and vault-based activation.

use std::sync::OnceLock;

/// A native backend that can be activated when its vault key is present.
pub struct ProviderEntry {
    pub id: &'static str,
    pub prefixes: &'static [&'static str],
    pub vault_component: &'static str,
    pub vault_symbols: &'static [&'static str],
}

pub static PROVIDERS: &[ProviderEntry] = &[
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
        vault_symbols: &["google-ai-studio-api-key", "gemini-api-key", "google-api-key"],
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
    ProviderEntry {
        id: "harmoniis",
        prefixes: &["ber1-ai/"],
        vault_component: "harmoniis-backend",
        vault_symbols: &["harmoniis-api-key", "harmoniis-router-api-key", "harmoniis"],
    },
];

// ── Vault key detection (cached) ─────────────────────────────────────

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

pub fn active_providers() -> &'static Vec<&'static str> {
    ACTIVE_PROVIDERS.get_or_init(detect_active_providers)
}

fn has_vault_key(component: &str, symbols: &[&str]) -> bool {
    harmonia_provider_protocol::get_secret_any(component, symbols)
        .ok()
        .flatten()
        .is_some()
}

pub fn provider_is_active(provider_id: &str) -> bool {
    active_providers().contains(&provider_id)
}

pub fn resolve_provider(model: &str) -> Option<&'static ProviderEntry> {
    let lower = model.to_ascii_lowercase();
    PROVIDERS
        .iter()
        .find(|p| p.prefixes.iter().any(|prefix| lower.starts_with(prefix)))
}
