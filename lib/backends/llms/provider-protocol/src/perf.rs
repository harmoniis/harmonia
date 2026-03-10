//! Performance logging for model evolution.
//! Writes to the SQLite metrics database via the metrics module.

use crate::metrics;
use crate::offering::ModelOffering;

/// Log a model invocation's performance to the metrics database.
pub fn log_model_performance(
    pool: &[ModelOffering],
    backend: &str,
    model: &str,
    latency_ms: u128,
    success: bool,
) {
    let (usd_in, usd_out) = pool
        .iter()
        .find(|m| m.id == model)
        .map(|m| (m.usd_in_1k, m.usd_out_1k))
        .unwrap_or((0.0, 0.0));
    metrics::record_llm_perf(backend, model, latency_ms, success, usd_in, usd_out);
}
