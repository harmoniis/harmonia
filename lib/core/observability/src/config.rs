//! Config-store integration for observability settings.

use crate::model::TraceLevel;

const COMPONENT: &str = "observability";

/// Read observability configuration from config-store + vault + env vars.
pub(crate) struct ObservabilityConfig {
    pub enabled: bool,
    pub trace_level: TraceLevel,
    pub sample_rate: f64,
    pub project_name: String,
    pub api_url: String,
    pub api_key: String,
}

impl ObservabilityConfig {
    /// Load configuration with fallback chain: config-store -> env var -> default.
    pub fn load() -> Self {
        let enabled = read_config_or_env("enabled", "HARMONIA_OBSERVABILITY_ENABLED")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let trace_level = read_config_or_env("trace-level", "HARMONIA_OBSERVABILITY_TRACE_LEVEL")
            .map(|v| TraceLevel::from_str(&v))
            .unwrap_or(TraceLevel::Standard);

        let sample_rate = read_config_or_env("sample-rate", "HARMONIA_OBSERVABILITY_SAMPLE_RATE")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);

        let project_name =
            read_config_or_env("project-name", "HARMONIA_OBSERVABILITY_PROJECT_NAME")
                .unwrap_or_else(|| "harmonia".to_string());

        let api_url = read_config_or_env("api-url", "HARMONIA_OBSERVABILITY_API_URL")
            .unwrap_or_else(|| "https://api.smith.langchain.com".to_string());

        // API key: vault first, then env var
        let api_key = read_vault_key()
            .or_else(|| std::env::var("LANGCHAIN_API_KEY").ok())
            .unwrap_or_default();

        Self {
            enabled,
            trace_level,
            sample_rate,
            project_name,
            api_url,
            api_key,
        }
    }
}

fn read_config_or_env(key: &str, env_var: &str) -> Option<String> {
    // Try config-store first
    if let Ok(Some(val)) = harmonia_config_store::get_config(COMPONENT, COMPONENT, key) {
        if !val.is_empty() {
            return Some(val);
        }
    }

    // Fall back to env var
    std::env::var(env_var).ok().filter(|v| !v.is_empty())
}

fn read_vault_key() -> Option<String> {
    harmonia_vault::get_secret_for_component("observability", "langsmith-api-key")
        .ok()
        .flatten()
}
