use std::collections::HashMap;
use std::env;

use crate::store::{normalize_env_symbol, normalize_symbol};

fn parse_import_pairs(spec: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    for pair in spec.split(',') {
        let p = pair.trim();
        if p.is_empty() {
            continue;
        }
        if let Some((env_name, symbols)) = p.split_once('=') {
            let env_name = env_name.trim();
            if env_name.is_empty() {
                continue;
            }
            let mut keys = Vec::new();
            for sym in symbols.split('|') {
                let s = normalize_symbol(sym);
                if !s.is_empty() {
                    keys.push(s);
                }
            }
            if !keys.is_empty() {
                out.push((env_name.to_string(), keys));
            }
        }
    }
    out
}

pub fn ingest_env(secrets: &mut HashMap<String, String>) {
    // Generic ingest path 1: prefixed env vars become direct symbols.
    for (k, v) in env::vars() {
        if let Some(symbol_raw) = k.strip_prefix("HARMONIA_VAULT_SECRET__") {
            let symbol = normalize_env_symbol(symbol_raw);
            if !symbol.is_empty() {
                secrets.insert(symbol, v);
            }
        }
    }

    // Well-known provider env vars → vault symbols.
    // These follow the standard `PROVIDER_API_KEY` naming convention.
    let well_known = [
        ("OPENROUTER_API_KEY", "openrouter-api-key"),
        ("HARMONIIS_ROUTER_API_KEY", "harmoniis-api-key"),
        ("GROQ_API_KEY", "groq-api-key"),
        ("ANTHROPIC_API_KEY", "anthropic-api-key"),
        ("OPENAI_API_KEY", "openai-api-key"),
        ("XAI_API_KEY", "xai-api-key"),
    ];
    for (env_name, symbol) in &well_known {
        if let Ok(value) = env::var(env_name) {
            if !value.is_empty() && !secrets.contains_key(*symbol) {
                secrets.insert(symbol.to_string(), value);
            }
        }
    }

    // Generic ingest path 2: operator-provided import map, no key names hardcoded in code.
    // Format: ENV_NAME=symbol_one|symbol_two,OTHER_ENV=other_symbol
    if let Ok(spec) = env::var("HARMONIA_VAULT_IMPORT") {
        for (env_name, symbols) in parse_import_pairs(&spec) {
            if let Ok(value) = env::var(&env_name) {
                for symbol in symbols {
                    secrets.insert(symbol, value.clone());
                }
            }
        }
    }
}
