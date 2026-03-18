use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

use crate::ingest::ingest_env;
use crate::state::secrets;
use crate::store::{
    derive_scoped_secret_hex, has_symbol, list_symbols, load_legacy_kv_into_db_if_present,
    load_store_file, normalize_symbol, upsert_secret,
};

static COMPONENT_POLICY_OVERRIDES: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();

fn load_all_sources(map: &mut HashMap<String, String>) {
    map.clear();
    for (k, v) in load_store_file() {
        map.insert(k, v);
    }
    ingest_env(map);
}

fn default_component_patterns(component: &str) -> &'static [&'static str] {
    match component {
        "openrouter-backend" => &["openrouter", "openrouter-api-key"],
        "openai-backend" => &["openai", "openai-api-key"],
        "anthropic-backend" => &["anthropic", "anthropic-api-key"],
        "xai-backend" => &["xai", "xai-api-key", "x-ai-api-key"],
        "google-ai-studio-backend" => &[
            "google-ai-studio-api-key",
            "gemini-api-key",
            "google-api-key",
        ],
        "google-vertex-backend" => &[
            "google-vertex-access-token",
            "vertex-access-token",
            "google-vertex-project-id",
            "vertex-project-id",
            "google-vertex-location",
            "vertex-location",
        ],
        "amazon-bedrock-backend" => &[
            "aws-access-key-id",
            "aws-secret-access-key",
            "aws-session-token",
            "aws-region",
        ],
        "groq-backend" => &["groq", "groq-api-key"],
        "alibaba-backend" => &["alibaba", "alibaba-api-key", "dashscope-api-key"],
        "search-exa-tool" => &["exa-api-key"],
        "search-brave-tool" => &["brave-api-key"],
        "whisper-backend" => &["groq-api-key", "groq", "openai-api-key", "openai"],
        "elevenlabs-backend" => &["elevenlabs-api-key", "elevenlabs"],
        "email-frontend" => &[
            "email-imap-password",
            "email-password",
            "email-smtp-password",
            "email-api-key",
        ],
        "mattermost-frontend" => &["mattermost-bot-token", "mattermost-token"],
        "nostr-frontend" => &["nostr-private-key", "nostr-nsec"],
        "telegram-frontend" => &["telegram-bot-token", "telegram-bot-api-token"],
        "slack-frontend" => &[
            "slack-bot-token",
            "slack-app-token",
            "slack-bot-token-v2",
            "slack-app-level-token",
        ],
        "discord-frontend" => &["discord-bot-token", "discord-token"],
        "signal-frontend" => &[
            "signal-auth-token",
            "signal-auth-token-v2",
            "signal-account",
            "signal-rpc-url",
            "signal-bridge-url",
        ],
        "whatsapp-frontend" => &[
            "whatsapp-session",
            "whatsapp-api-key",
            "whatsapp-bridge-url",
        ],
        "imessage-frontend" => &[
            "bluebubbles-password",
            "imessage-password",
            "bluebubbles-server-url",
            "imessage-server-url",
        ],
        "tailscale-frontend" => &["tailscale-auth-key"],
        "mqtt-frontend" => &[
            "mqtt-agent-fp",
            "mqtt-tls-master-seed",
            "mqtt-tls-client-cert-pem",
            "mqtt-tls-client-key-pem",
            "mqtt-tls-client-cert-path",
            "mqtt-tls-client-key-path",
            "mqtt-broker-url",
        ],
        "admin-intent" => &["*pubkey"],
        "parallel-agents-core" => &[
            "openrouter",
            "openrouter-api-key",
            "exa-api-key",
            "brave-api-key",
        ],
        "observability" => &["langsmith-api-key"],
        _ => &[],
    }
}

fn parse_component_policy_env() -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    let raw = match env::var("HARMONIA_VAULT_COMPONENT_POLICY") {
        Ok(v) => v,
        Err(_) => return out,
    };

    // Format:
    // component=pattern1,pattern2;component2=pattern3
    for entry in raw.split(';') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (component, rhs) = match entry.split_once('=') {
            Some(v) => v,
            None => continue,
        };
        let component = component.trim().to_ascii_lowercase();
        if component.is_empty() {
            continue;
        }
        let patterns: Vec<String> = rhs
            .split(',')
            .map(|s| normalize_symbol(s))
            .filter(|s| !s.is_empty())
            .collect();
        if !patterns.is_empty() {
            out.insert(component, patterns);
        }
    }

    out
}

fn component_policy_overrides() -> &'static HashMap<String, Vec<String>> {
    COMPONENT_POLICY_OVERRIDES.get_or_init(parse_component_policy_env)
}

fn pattern_matches(pattern: &str, symbol: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return symbol.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return symbol.ends_with(suffix);
    }
    symbol == pattern
}

fn component_can_read_symbol(component: &str, symbol: &str) -> bool {
    let component = component.trim().to_ascii_lowercase();
    let symbol = normalize_symbol(symbol);

    // Built-in defaults
    let mut allowed = default_component_patterns(&component)
        .iter()
        .any(|pat| pattern_matches(pat, &symbol));

    // Env overrides (component-specific and global "*")
    let overrides = component_policy_overrides();
    for key in [component.as_str(), "*"] {
        if let Some(patterns) = overrides.get(key) {
            if patterns.iter().any(|pat| pattern_matches(pat, &symbol)) {
                allowed = true;
            }
        }
    }

    allowed
}

pub fn init_from_env() -> Result<(), String> {
    load_legacy_kv_into_db_if_present()?;
    let mut map = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;
    load_all_sources(&mut map);
    Ok(())
}

pub fn get_secret_for_component(component: &str, symbol: &str) -> Result<Option<String>, String> {
    if !component_can_read_symbol(component, symbol) {
        return Err(format!(
            "vault policy denied component '{}' for symbol '{}'",
            component,
            normalize_symbol(symbol)
        ));
    }
    let normalized = normalize_symbol(symbol);
    Ok(secrets()
        .read()
        .ok()
        .and_then(|map| map.get(&normalized).cloned()))
}

pub fn set_secret_for_symbol(symbol: &str, value: &str) -> Result<(), String> {
    let key = normalize_symbol(symbol);
    let mut map = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;
    map.insert(key, value.to_string());
    upsert_secret(symbol, value)
}

pub fn derive_component_seed_hex(component: &str, purpose: &str) -> Result<String, String> {
    let component = component.trim().to_ascii_lowercase();
    if component.is_empty() {
        return Err("component cannot be empty".to_string());
    }
    let purpose = purpose.trim();
    if purpose.is_empty() {
        return Err("purpose cannot be empty".to_string());
    }
    let scope = format!("component/{component}/{purpose}");
    derive_scoped_secret_hex(&scope)
}

pub fn has_secret_for_symbol(symbol: &str) -> bool {
    if let Ok(map) = secrets().read() {
        return map.contains_key(&normalize_symbol(symbol));
    }
    has_symbol(symbol).unwrap_or(false)
}

pub fn list_secret_symbols() -> Vec<String> {
    if let Ok(map) = secrets().read() {
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        keys.dedup();
        return keys;
    }
    list_symbols().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_symbol_lookup() {
        {
            let mut map = crate::state::secrets().write().unwrap();
            map.insert("openrouter".to_string(), "k".to_string());
        }
        let got = get_secret_for_component("openrouter-backend", ":OpenRouter")
            .unwrap()
            .unwrap();
        assert_eq!(got, "k");
    }

    #[test]
    fn unknown_component_is_denied() {
        let denied = get_secret_for_component("random-component", "openrouter");
        assert!(denied.is_err());
    }
}
