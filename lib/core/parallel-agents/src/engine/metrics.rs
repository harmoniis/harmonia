//! Metrics functions for parallel agents.
//! Cost estimation uses in-memory price map; reports query the SQLite metrics DB.

use crate::model::{state, ModelPrice};

fn estimate_tokens(s: &str) -> f64 {
    (s.chars().count() as f64 / 4.0).max(1.0)
}

pub(super) fn estimate_cost(model: &str, prompt: &str, response: &str) -> f64 {
    let st = match state().read() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };
    let p: ModelPrice = match st.prices.get(model) {
        Some(v) => *v,
        None => return 0.0,
    };
    let in_k = estimate_tokens(prompt) / 1000.0;
    let out_k = estimate_tokens(response) / 1000.0;
    p.usd_per_1k_input * in_k + p.usd_per_1k_output * out_k
}

/// Performance report — queries the SQLite metrics database.
pub(super) fn render_report() -> String {
    harmonia_provider_protocol::query_performance_report()
}
