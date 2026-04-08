/// Config-driven parameters for the memory field engine.
/// All magic numbers flow from config-store with sensible defaults.

pub fn cfg_f64(key: &str, default: f64) -> f64 {
    harmonia_config_store::get_own("memory-field", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

pub fn cfg_i64(key: &str, default: i64) -> i64 {
    harmonia_config_store::get_own("memory-field", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default)
}
