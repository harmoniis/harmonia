//! Model seed policy configuration — unified pool setup.

use console::style;
use dialoguer::Input;

use super::providers::llm_provider_defs;

fn default_seed_models_for_provider(provider_id: &str) -> Vec<&'static str> {
    match provider_id {
        "harmoniis" => vec![
            "ber1-ai/qwen3.5-27b",
            "ber1-ai/magistral-24b",
            "ber1-ai/nanbeige-3b",
        ],
        "openrouter" => vec![
            "qwen/qwen3.6-plus:free",
            "google/gemini-2.5-flash-lite-preview-09-2025",
            "x-ai/grok-4.1-fast",
            "inception/mercury-2",
            "qwen/qwen3.5-flash-02-23",
            "minimax/minimax-m2.5",
        ],
        "xai" => vec!["x-ai/grok-4.20"],
        "anthropic" => vec!["anthropic/claude-opus-4.6"],
        "google-ai-studio" | "google-vertex" => {
            vec!["google/gemini-2.5-flash-lite-preview-09-2025"]
        }
        "bedrock" => vec!["amazon/nova-micro-v1", "amazon/nova-lite-v1"],
        "groq" => vec!["qwen/qwen3.6-plus:free"],
        "alibaba" => vec!["qwen/qwen3.6-plus:free"],
        _ => vec![],
    }
}

fn all_provider_seed_defaults() -> Vec<(&'static str, Vec<&'static str>)> {
    vec![
        ("openrouter", default_seed_models_for_provider("openrouter")),
        ("openai", default_seed_models_for_provider("openai")),
        ("anthropic", default_seed_models_for_provider("anthropic")),
        ("xai", default_seed_models_for_provider("xai")),
        (
            "google-ai-studio",
            default_seed_models_for_provider("google-ai-studio"),
        ),
        (
            "google-vertex",
            default_seed_models_for_provider("google-vertex"),
        ),
        ("bedrock", default_seed_models_for_provider("bedrock")),
        ("groq", default_seed_models_for_provider("groq")),
        ("alibaba", default_seed_models_for_provider("alibaba")),
        ("harmoniis", default_seed_models_for_provider("harmoniis")),
    ]
}

fn normalize_model_csv(raw: &str) -> String {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

fn seed_provider_ids(configured_provider_ids: &[String]) -> Vec<String> {
    let defs = llm_provider_defs();
    let mut provider_ids: Vec<String> = Vec::new();

    let mut push_known = |id: &str| {
        let known = defs.iter().any(|d| d.id == id);
        if known && !provider_ids.iter().any(|existing| existing == id) {
            provider_ids.push(id.to_string());
        }
    };

    for id in configured_provider_ids {
        push_known(id);
    }

    if let Ok(Some(active_provider)) =
        harmonia_config_store::get_config("harmonia-cli", "model-policy", "provider")
    {
        push_known(&active_provider);
    }

    for (provider, _) in all_provider_seed_defaults() {
        push_known(provider);
    }

    if provider_ids.is_empty() {
        for def in defs {
            provider_ids.push(def.id.to_string());
        }
    }

    provider_ids
}

pub(crate) fn configure_model_seed_policy(
    configured_provider_ids: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    let provider_ids = seed_provider_ids(configured_provider_ids);
    if provider_ids.is_empty() {
        return Err("at least one provider must be available for seed policy".into());
    }

    let defs = llm_provider_defs();
    let _provider_labels: Vec<String> = provider_ids
        .iter()
        .map(|id| {
            defs.iter()
                .find(|d| d.id == id)
                .map(|d| d.display.to_string())
                .unwrap_or_else(|| id.clone())
        })
        .collect();

    let mut unified_seeds: Vec<String> = Vec::new();
    for (provider, defaults) in all_provider_seed_defaults() {
        if provider == "harmoniis" && provider_ids.contains(&provider.to_string()) {
            for m in &defaults {
                unified_seeds.push(m.to_string());
            }
        }
    }
    for (provider, defaults) in all_provider_seed_defaults() {
        if provider != "harmoniis" && provider_ids.contains(&provider.to_string()) {
            for m in &defaults {
                if !unified_seeds.contains(&m.to_string()) {
                    unified_seeds.push(m.to_string());
                }
            }
        }
    }
    let unified_csv = unified_seeds.join(",");
    let entered_seed_csv: String = Input::new()
        .with_prompt("  Default models pool")
        .default(unified_csv.clone())
        .interact_text()?;
    let normalized_seed_csv = {
        let n = normalize_model_csv(&entered_seed_csv);
        if n.is_empty() {
            unified_csv
        } else {
            n
        }
    };

    let cs = |scope: &str, key: &str, val: &str| -> Result<(), Box<dyn std::error::Error>> {
        harmonia_config_store::set_config("harmonia-cli", scope, key, val).map_err(|e| e.into())
    };

    cs("model-policy", "provider", "unified")?;
    cs("model-policy", "seed-models", &normalized_seed_csv)?;

    for (provider, defaults) in all_provider_seed_defaults() {
        let key = format!("seed-models-{}", provider);
        let csv = defaults.join(",");
        cs("model-policy", &key, &csv)?;
    }

    println!(
        "    {} Default models pool stored (models={})",
        style("✓").green().bold(),
        normalized_seed_csv
    );

    Ok(())
}
