//! Model offering types and pool-based selection.

use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A model available from a provider backend, with hardcoded pricing.
#[derive(Debug, Clone)]
pub struct ModelOffering {
    pub id: &'static str,
    pub tier: &'static str,
    pub usd_in_1k: f64,
    pub usd_out_1k: f64,
    pub quality: u8,
    pub speed: u8,
    pub tags: &'static [&'static str],
}

/// Global round-robin index for pool selection.
static POOL_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Select a model from `pool` for the given task category.
/// Round-robins through eligible models to gather performance data.
///
/// Task hints: `"orchestration"`, `"execution"`, `"memory-ops"`, `"coding"`,
///             `"reasoning"`, `"casual"`, `"software-dev"`, or `""` for any.
pub fn select_from_pool(pool: &[ModelOffering], task_hint: &str) -> String {
    if pool.is_empty() {
        return String::new();
    }
    let hint = task_hint.trim().to_ascii_lowercase();
    let eligible: Vec<&ModelOffering> = if hint.is_empty() {
        pool.iter()
            .filter(|m| matches!(m.tier, "free" | "micro" | "lite"))
            .collect()
    } else {
        let matched: Vec<&ModelOffering> = pool
            .iter()
            .filter(|m| m.tags.iter().any(|t| *t == hint.as_str()))
            .collect();
        if matched.is_empty() {
            pool.iter()
                .filter(|m| matches!(m.tier, "free" | "micro" | "lite"))
                .collect()
        } else {
            matched
        }
    };
    if eligible.is_empty() {
        return pool[0].id.to_string();
    }
    let idx = POOL_INDEX.fetch_add(1, Ordering::Relaxed) % eligible.len();
    eligible[idx].id.to_string()
}

/// Return pool models as fallback candidates (cheapest first, excluding `primary`).
pub fn pool_fallbacks(pool: &[ModelOffering], primary: &str) -> Vec<String> {
    pool.iter()
        .filter(|m| m.id != primary && matches!(m.tier, "free" | "micro" | "lite"))
        .map(|m| m.id.to_string())
        .collect()
}

/// Serialise the entire offerings pool to a JSON array.
pub fn offerings_to_json(pool: &[ModelOffering]) -> String {
    let arr: Vec<Value> = pool
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "tier": m.tier,
                "usd_in_1k": m.usd_in_1k,
                "usd_out_1k": m.usd_out_1k,
                "quality": m.quality,
                "speed": m.speed,
                "tags": m.tags,
            })
        })
        .collect();
    serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// Strip the `provider/` prefix from a model ID (e.g. `"openai/gpt-5"` → `"gpt-5"`).
pub fn strip_provider_prefix(model: &str) -> String {
    model
        .trim()
        .split_once('/')
        .map(|(_, tail)| tail.to_string())
        .unwrap_or_else(|| model.trim().to_string())
}
