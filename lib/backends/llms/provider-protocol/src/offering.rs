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
    // Performance-aware selection: rank by success_rate / avg_latency.
    // Models that fail or are slow get ranked lower automatically.
    select_best_by_performance(&eligible)
}

/// Rank eligible models by historical performance. Best success rate with
/// lowest latency wins. Falls back to round-robin if no perf data.
fn select_best_by_performance(eligible: &[&ModelOffering]) -> String {
    let mut scored: Vec<(&ModelOffering, f64)> = eligible
        .iter()
        .map(|m| {
            let stats = crate::metrics::query_model_stats(m.id);
            // Parse success-rate and avg-latency from sexp
            let sr = extract_f64_from_sexp(&stats, "success-rate").unwrap_or(0.5);
            let lat = extract_f64_from_sexp(&stats, "avg-latency-ms").unwrap_or(5000.0);
            let count = extract_f64_from_sexp(&stats, "count").unwrap_or(0.0);
            // Score: success_rate * speed_factor * (1 + log(count+1))
            // More data = more confidence. Faster = better. Higher success = better.
            let speed_factor = 10000.0 / (lat + 1000.0); // 1.0 at 10s, 5.0 at 1s
            let confidence = (1.0 + (count + 1.0).ln()).min(3.0);
            let score = sr * speed_factor * confidence;
            (*m, score)
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    if let Some((best, _)) = scored.first() {
        best.id.to_string()
    } else {
        // Fallback: round-robin
        let idx = POOL_INDEX.fetch_add(1, Ordering::Relaxed) % eligible.len();
        eligible[idx].id.to_string()
    }
}

fn extract_f64_from_sexp(sexp: &str, key: &str) -> Option<f64> {
    let search = format!(":{} ", key);
    let pos = sexp.find(&search)?;
    let start = pos + search.len();
    let rest = &sexp[start..];
    let end = rest.find(|c: char| c.is_whitespace() || c == ')').unwrap_or(rest.len());
    rest[..end].parse::<f64>().ok()
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
