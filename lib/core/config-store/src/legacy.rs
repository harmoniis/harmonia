/// Maps (scope, key) pairs to legacy environment variable names that don't follow
/// the canonical `HARMONIA_{SCOPE}_{KEY}` pattern.

pub(crate) fn legacy_env_name(scope: &str, key: &str) -> Option<&'static str> {
    match (scope, key) {
        // ── global ──
        ("global", "state-root") => Some("HARMONIA_STATE_ROOT"),
        ("global", "source-dir") => Some("HARMONIA_SOURCE_DIR"),
        ("global", "lib-dir") => Some("HARMONIA_LIB_DIR"),
        ("global", "env") => Some("HARMONIA_ENV"),
        ("global", "fs-root") => Some("HARMONIA_FS_ROOT"),
        ("global", "metrics-db") => Some("HARMONIA_METRICS_DB"),
        ("global", "recovery-log") => Some("HARMONIA_RECOVERY_LOG"),

        // ── LLM backends ──
        ("openai-backend", "base-url") => Some("HARMONIA_OPENAI_BASE_URL"),
        ("openai-backend", "connect-timeout-secs") => Some("HARMONIA_OPENAI_CONNECT_TIMEOUT_SECS"),
        ("openai-backend", "max-time-secs") => Some("HARMONIA_OPENAI_MAX_TIME_SECS"),
        ("anthropic-backend", "api-version") => Some("HARMONIA_ANTHROPIC_VERSION"),
        ("anthropic-backend", "max-tokens") => Some("HARMONIA_ANTHROPIC_MAX_TOKENS"),
        ("anthropic-backend", "connect-timeout-secs") => {
            Some("HARMONIA_ANTHROPIC_CONNECT_TIMEOUT_SECS")
        }
        ("anthropic-backend", "max-time-secs") => Some("HARMONIA_ANTHROPIC_MAX_TIME_SECS"),
        ("xai-backend", "base-url") => Some("HARMONIA_XAI_BASE_URL"),
        ("xai-backend", "connect-timeout-secs") => Some("HARMONIA_XAI_CONNECT_TIMEOUT_SECS"),
        ("xai-backend", "max-time-secs") => Some("HARMONIA_XAI_MAX_TIME_SECS"),
        ("groq-backend", "base-url") => Some("HARMONIA_GROQ_BASE_URL"),
        ("groq-backend", "connect-timeout-secs") => Some("HARMONIA_GROQ_CONNECT_TIMEOUT_SECS"),
        ("groq-backend", "max-time-secs") => Some("HARMONIA_GROQ_MAX_TIME_SECS"),
        ("alibaba-backend", "base-url") => Some("HARMONIA_ALIBABA_BASE_URL"),
        ("alibaba-backend", "connect-timeout-secs") => {
            Some("HARMONIA_ALIBABA_CONNECT_TIMEOUT_SECS")
        }
        ("alibaba-backend", "max-time-secs") => Some("HARMONIA_ALIBABA_MAX_TIME_SECS"),
        ("google-ai-studio-backend", "base-url") => Some("HARMONIA_GOOGLE_AI_STUDIO_BASE_URL"),
        ("google-ai-studio-backend", "connect-timeout-secs") => {
            Some("HARMONIA_GOOGLE_AI_STUDIO_CONNECT_TIMEOUT_SECS")
        }
        ("google-ai-studio-backend", "max-time-secs") => {
            Some("HARMONIA_GOOGLE_AI_STUDIO_MAX_TIME_SECS")
        }
        ("google-vertex-backend", "project-id") => Some("HARMONIA_GOOGLE_VERTEX_PROJECT_ID"),
        ("google-vertex-backend", "location") => Some("HARMONIA_GOOGLE_VERTEX_LOCATION"),
        ("google-vertex-backend", "endpoint") => Some("HARMONIA_GOOGLE_VERTEX_ENDPOINT"),
        ("google-vertex-backend", "connect-timeout-secs") => {
            Some("HARMONIA_GOOGLE_VERTEX_CONNECT_TIMEOUT_SECS")
        }
        ("google-vertex-backend", "max-time-secs") => Some("HARMONIA_GOOGLE_VERTEX_MAX_TIME_SECS"),
        ("amazon-bedrock-backend", "region") => Some("HARMONIA_BEDROCK_REGION"),
        ("openrouter-backend", "connect-timeout-secs") => {
            Some("HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS")
        }
        ("openrouter-backend", "max-time-secs") => Some("HARMONIA_OPENROUTER_MAX_TIME_SECS"),

        // ── tools ──
        ("whisper-tool", "api-url") => Some("HARMONIA_WHISPER_API_URL"),
        ("whisper-tool", "model") => Some("HARMONIA_WHISPER_MODEL"),
        ("elevenlabs-tool", "api-url") => Some("HARMONIA_ELEVENLABS_API_URL"),
        ("search-exa-tool", "api-url") => Some("HARMONIA_EXA_API_URL"),
        ("search-brave-tool", "api-url") => Some("HARMONIA_BRAVE_API_URL"),

        // ── frontends ──
        ("mqtt-frontend", "broker") => Some("HARMONIA_MQTT_BROKER"),
        ("mqtt-frontend", "timeout-ms") => Some("HARMONIA_MQTT_TIMEOUT_MS"),
        ("mqtt-frontend", "tls") => Some("HARMONIA_MQTT_TLS"),
        ("mqtt-frontend", "ca-cert") => Some("HARMONIA_MQTT_CA_CERT"),
        ("mqtt-frontend", "client-cert") => Some("HARMONIA_MQTT_CLIENT_CERT"),
        ("mqtt-frontend", "client-key") => Some("HARMONIA_MQTT_CLIENT_KEY"),
        ("email-frontend", "api-url") => Some("HARMONIA_EMAIL_API_URL"),
        ("email-frontend", "from") => Some("HARMONIA_EMAIL_FROM"),
        ("email-frontend", "default-subject") => Some("HARMONIA_EMAIL_DEFAULT_SUBJECT"),
        ("push-frontend", "mode") => Some("HARMONIA_PUSH_MODE"),
        ("push-frontend", "log") => Some("HARMONIA_PUSH_LOG"),
        ("nostr-frontend", "api-url") => Some("HARMONIA_NOSTR_API_URL"),
        ("mattermost-frontend", "api-url") => Some("HARMONIA_MATTERMOST_API_URL"),
        ("whatsapp-frontend", "api-url") => Some("HARMONIA_WHATSAPP_API_URL"),
        ("imessage-frontend", "server-url") => Some("HARMONIA_IMESSAGE_SERVER_URL"),
        ("discord-frontend", "channels") => Some("HARMONIA_DISCORD_CHANNELS"),
        ("slack-frontend", "channels") => Some("HARMONIA_SLACK_CHANNELS"),
        ("signal-frontend", "rpc-url") => Some("HARMONIA_SIGNAL_RPC_URL"),
        ("signal-frontend", "account") => Some("HARMONIA_SIGNAL_ACCOUNT"),

        // ── core ──
        ("harmonic-matrix", "store-kind") => Some("HARMONIA_MATRIX_STORE_KIND"),
        ("harmonic-matrix", "db") => Some("HARMONIA_MATRIX_DB"),
        ("harmonic-matrix", "graph-uri") => Some("HARMONIA_MATRIX_GRAPH_URI"),
        ("harmonic-matrix", "history-limit") => Some("HARMONIA_MATRIX_HISTORY_LIMIT"),
        ("phoenix-core", "recovery-log") => Some("HARMONIA_RECOVERY_LOG"),
        ("phoenix-core", "env") => Some("HARMONIA_ENV"),
        ("tailnet-core", "port") => Some("HARMONIA_TAILNET_PORT"),
        ("tailnet-core", "hostname-prefix") => Some("HARMONIA_TAILNET_HOSTNAME_PREFIX"),
        ("ouroboros-core", "patch-dir") => Some("HARMONIA_OUROBOROS_PATCH_DIR"),
        ("s3-storage", "mode") => Some("HARMONIA_S3_MODE"),
        ("s3-storage", "local-root") => Some("HARMONIA_S3_LOCAL_ROOT"),

        // ── evolution ──
        ("evolution", "mode") => Some("HARMONIA_EVOLUTION_MODE"),
        ("evolution", "source-rewrite-enabled") => Some("HARMONIA_SOURCE_REWRITE_ENABLED"),
        ("evolution", "distributed-enabled") => Some("HARMONIA_DISTRIBUTED_EVOLUTION_ENABLED"),
        ("evolution", "distributed-store-kind") => Some("HARMONIA_DISTRIBUTED_STORE_KIND"),
        ("evolution", "distributed-store-bucket") => Some("HARMONIA_DISTRIBUTED_STORE_BUCKET"),
        ("evolution", "distributed-store-prefix") => Some("HARMONIA_DISTRIBUTED_STORE_PREFIX"),

        // ── policies / orchestration ──
        ("global", "system-dir") => Some("HARMONIA_SYSTEM_DIR"),
        ("harmonic-matrix", "route-signal-default") => Some("HARMONIA_ROUTE_SIGNAL_DEFAULT"),
        ("harmonic-matrix", "route-noise-default") => Some("HARMONIA_ROUTE_NOISE_DEFAULT"),
        ("harmonic-matrix", "topology-path") => Some("HARMONIA_MATRIX_TOPOLOGY_PATH"),
        ("model-policy", "path") => Some("HARMONIA_MODEL_POLICY_PATH"),
        ("model-policy", "planner") => Some("HARMONIA_MODEL_PLANNER"),
        ("model-policy", "planner-model") => Some("HARMONIA_MODEL_PLANNER_MODEL"),
        ("harmony-policy", "path") => Some("HARMONIA_HARMONY_POLICY_PATH"),
        ("parallel-agents-core", "policy-path") => Some("HARMONIA_PARALLEL_POLICY_PATH"),
        ("phoenix-core", "allow-prod-genesis") => Some("HARMONIA_ALLOW_PROD_GENESIS"),
        ("elevenlabs-tool", "default-voice") => Some("HARMONIA_ELEVENLABS_DEFAULT_VOICE"),
        ("elevenlabs-tool", "default-output-path") => {
            Some("HARMONIA_ELEVENLABS_DEFAULT_OUTPUT_PATH")
        }

        _ => None,
    }
}

/// Build the canonical env var name: `HARMONIA_{SCOPE}_{KEY}` uppercased, hyphens → underscores.
pub(crate) fn canonical_env_name(scope: &str, key: &str) -> String {
    format!(
        "HARMONIA_{}_{}",
        scope.to_ascii_uppercase().replace('-', "_"),
        key.to_ascii_uppercase().replace('-', "_"),
    )
}

/// Returns all known (scope, key, legacy_env_name) triples for seeding.
pub(crate) fn all_legacy_entries() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("global", "state-root", "HARMONIA_STATE_ROOT"),
        ("global", "source-dir", "HARMONIA_SOURCE_DIR"),
        ("global", "lib-dir", "HARMONIA_LIB_DIR"),
        ("global", "env", "HARMONIA_ENV"),
        ("global", "fs-root", "HARMONIA_FS_ROOT"),
        ("global", "metrics-db", "HARMONIA_METRICS_DB"),
        ("global", "recovery-log", "HARMONIA_RECOVERY_LOG"),
        ("openai-backend", "base-url", "HARMONIA_OPENAI_BASE_URL"),
        (
            "openai-backend",
            "connect-timeout-secs",
            "HARMONIA_OPENAI_CONNECT_TIMEOUT_SECS",
        ),
        (
            "openai-backend",
            "max-time-secs",
            "HARMONIA_OPENAI_MAX_TIME_SECS",
        ),
        (
            "anthropic-backend",
            "api-version",
            "HARMONIA_ANTHROPIC_VERSION",
        ),
        (
            "anthropic-backend",
            "max-tokens",
            "HARMONIA_ANTHROPIC_MAX_TOKENS",
        ),
        (
            "anthropic-backend",
            "connect-timeout-secs",
            "HARMONIA_ANTHROPIC_CONNECT_TIMEOUT_SECS",
        ),
        (
            "anthropic-backend",
            "max-time-secs",
            "HARMONIA_ANTHROPIC_MAX_TIME_SECS",
        ),
        ("xai-backend", "base-url", "HARMONIA_XAI_BASE_URL"),
        (
            "xai-backend",
            "connect-timeout-secs",
            "HARMONIA_XAI_CONNECT_TIMEOUT_SECS",
        ),
        ("xai-backend", "max-time-secs", "HARMONIA_XAI_MAX_TIME_SECS"),
        ("groq-backend", "base-url", "HARMONIA_GROQ_BASE_URL"),
        (
            "groq-backend",
            "connect-timeout-secs",
            "HARMONIA_GROQ_CONNECT_TIMEOUT_SECS",
        ),
        (
            "groq-backend",
            "max-time-secs",
            "HARMONIA_GROQ_MAX_TIME_SECS",
        ),
        ("alibaba-backend", "base-url", "HARMONIA_ALIBABA_BASE_URL"),
        (
            "alibaba-backend",
            "connect-timeout-secs",
            "HARMONIA_ALIBABA_CONNECT_TIMEOUT_SECS",
        ),
        (
            "alibaba-backend",
            "max-time-secs",
            "HARMONIA_ALIBABA_MAX_TIME_SECS",
        ),
        (
            "google-ai-studio-backend",
            "base-url",
            "HARMONIA_GOOGLE_AI_STUDIO_BASE_URL",
        ),
        (
            "google-ai-studio-backend",
            "connect-timeout-secs",
            "HARMONIA_GOOGLE_AI_STUDIO_CONNECT_TIMEOUT_SECS",
        ),
        (
            "google-ai-studio-backend",
            "max-time-secs",
            "HARMONIA_GOOGLE_AI_STUDIO_MAX_TIME_SECS",
        ),
        (
            "google-vertex-backend",
            "project-id",
            "HARMONIA_GOOGLE_VERTEX_PROJECT_ID",
        ),
        (
            "google-vertex-backend",
            "location",
            "HARMONIA_GOOGLE_VERTEX_LOCATION",
        ),
        (
            "google-vertex-backend",
            "endpoint",
            "HARMONIA_GOOGLE_VERTEX_ENDPOINT",
        ),
        (
            "google-vertex-backend",
            "connect-timeout-secs",
            "HARMONIA_GOOGLE_VERTEX_CONNECT_TIMEOUT_SECS",
        ),
        (
            "google-vertex-backend",
            "max-time-secs",
            "HARMONIA_GOOGLE_VERTEX_MAX_TIME_SECS",
        ),
        (
            "amazon-bedrock-backend",
            "region",
            "HARMONIA_BEDROCK_REGION",
        ),
        (
            "openrouter-backend",
            "connect-timeout-secs",
            "HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS",
        ),
        (
            "openrouter-backend",
            "max-time-secs",
            "HARMONIA_OPENROUTER_MAX_TIME_SECS",
        ),
        ("whisper-tool", "api-url", "HARMONIA_WHISPER_API_URL"),
        ("whisper-tool", "model", "HARMONIA_WHISPER_MODEL"),
        ("elevenlabs-tool", "api-url", "HARMONIA_ELEVENLABS_API_URL"),
        ("search-exa-tool", "api-url", "HARMONIA_EXA_API_URL"),
        ("search-brave-tool", "api-url", "HARMONIA_BRAVE_API_URL"),
        ("mqtt-frontend", "broker", "HARMONIA_MQTT_BROKER"),
        ("mqtt-frontend", "timeout-ms", "HARMONIA_MQTT_TIMEOUT_MS"),
        ("mqtt-frontend", "tls", "HARMONIA_MQTT_TLS"),
        ("mqtt-frontend", "ca-cert", "HARMONIA_MQTT_CA_CERT"),
        ("mqtt-frontend", "client-cert", "HARMONIA_MQTT_CLIENT_CERT"),
        ("mqtt-frontend", "client-key", "HARMONIA_MQTT_CLIENT_KEY"),
        ("email-frontend", "api-url", "HARMONIA_EMAIL_API_URL"),
        ("email-frontend", "from", "HARMONIA_EMAIL_FROM"),
        (
            "email-frontend",
            "default-subject",
            "HARMONIA_EMAIL_DEFAULT_SUBJECT",
        ),
        ("push-frontend", "mode", "HARMONIA_PUSH_MODE"),
        ("push-frontend", "log", "HARMONIA_PUSH_LOG"),
        ("nostr-frontend", "api-url", "HARMONIA_NOSTR_API_URL"),
        (
            "mattermost-frontend",
            "api-url",
            "HARMONIA_MATTERMOST_API_URL",
        ),
        ("whatsapp-frontend", "api-url", "HARMONIA_WHATSAPP_API_URL"),
        (
            "imessage-frontend",
            "server-url",
            "HARMONIA_IMESSAGE_SERVER_URL",
        ),
        ("discord-frontend", "channels", "HARMONIA_DISCORD_CHANNELS"),
        ("slack-frontend", "channels", "HARMONIA_SLACK_CHANNELS"),
        ("signal-frontend", "rpc-url", "HARMONIA_SIGNAL_RPC_URL"),
        ("signal-frontend", "account", "HARMONIA_SIGNAL_ACCOUNT"),
        (
            "harmonic-matrix",
            "store-kind",
            "HARMONIA_MATRIX_STORE_KIND",
        ),
        ("harmonic-matrix", "db", "HARMONIA_MATRIX_DB"),
        ("harmonic-matrix", "graph-uri", "HARMONIA_MATRIX_GRAPH_URI"),
        (
            "harmonic-matrix",
            "history-limit",
            "HARMONIA_MATRIX_HISTORY_LIMIT",
        ),
        ("phoenix-core", "recovery-log", "HARMONIA_RECOVERY_LOG"),
        ("phoenix-core", "env", "HARMONIA_ENV"),
        ("tailnet-core", "port", "HARMONIA_TAILNET_PORT"),
        (
            "tailnet-core",
            "hostname-prefix",
            "HARMONIA_TAILNET_HOSTNAME_PREFIX",
        ),
        (
            "ouroboros-core",
            "patch-dir",
            "HARMONIA_OUROBOROS_PATCH_DIR",
        ),
        ("s3-storage", "mode", "HARMONIA_S3_MODE"),
        ("s3-storage", "local-root", "HARMONIA_S3_LOCAL_ROOT"),
        ("evolution", "mode", "HARMONIA_EVOLUTION_MODE"),
        (
            "evolution",
            "source-rewrite-enabled",
            "HARMONIA_SOURCE_REWRITE_ENABLED",
        ),
        (
            "evolution",
            "distributed-enabled",
            "HARMONIA_DISTRIBUTED_EVOLUTION_ENABLED",
        ),
        (
            "evolution",
            "distributed-store-kind",
            "HARMONIA_DISTRIBUTED_STORE_KIND",
        ),
        (
            "evolution",
            "distributed-store-bucket",
            "HARMONIA_DISTRIBUTED_STORE_BUCKET",
        ),
        (
            "evolution",
            "distributed-store-prefix",
            "HARMONIA_DISTRIBUTED_STORE_PREFIX",
        ),
        ("global", "system-dir", "HARMONIA_SYSTEM_DIR"),
        (
            "harmonic-matrix",
            "route-signal-default",
            "HARMONIA_ROUTE_SIGNAL_DEFAULT",
        ),
        (
            "harmonic-matrix",
            "route-noise-default",
            "HARMONIA_ROUTE_NOISE_DEFAULT",
        ),
        (
            "harmonic-matrix",
            "topology-path",
            "HARMONIA_MATRIX_TOPOLOGY_PATH",
        ),
        ("model-policy", "path", "HARMONIA_MODEL_POLICY_PATH"),
        ("model-policy", "planner", "HARMONIA_MODEL_PLANNER"),
        (
            "model-policy",
            "planner-model",
            "HARMONIA_MODEL_PLANNER_MODEL",
        ),
        ("harmony-policy", "path", "HARMONIA_HARMONY_POLICY_PATH"),
        (
            "parallel-agents-core",
            "policy-path",
            "HARMONIA_PARALLEL_POLICY_PATH",
        ),
        (
            "phoenix-core",
            "allow-prod-genesis",
            "HARMONIA_ALLOW_PROD_GENESIS",
        ),
        (
            "elevenlabs-tool",
            "default-voice",
            "HARMONIA_ELEVENLABS_DEFAULT_VOICE",
        ),
        (
            "elevenlabs-tool",
            "default-output-path",
            "HARMONIA_ELEVENLABS_DEFAULT_OUTPUT_PATH",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_name_format() {
        assert_eq!(
            canonical_env_name("mqtt-frontend", "broker"),
            "HARMONIA_MQTT_FRONTEND_BROKER"
        );
        assert_eq!(
            canonical_env_name("openai-backend", "base-url"),
            "HARMONIA_OPENAI_BACKEND_BASE_URL"
        );
    }

    #[test]
    fn legacy_alias_lookup() {
        assert_eq!(
            legacy_env_name("mqtt-frontend", "broker"),
            Some("HARMONIA_MQTT_BROKER")
        );
        assert_eq!(legacy_env_name("unknown", "unknown"), None);
    }

    #[test]
    fn all_entries_consistent() {
        for (scope, key, env_name) in all_legacy_entries() {
            assert_eq!(
                legacy_env_name(scope, key),
                Some(env_name),
                "mismatch for ({scope}, {key})"
            );
        }
    }
}
