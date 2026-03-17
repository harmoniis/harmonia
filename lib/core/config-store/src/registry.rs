/// Declarative configuration registry — single source of truth for all known
/// (scope, key) pairs and their environment variable names.
///
/// Env var names are derived by three rules applied in order:
///   1. Global scope drops its name:  ("global", "state-root") → HARMONIA_STATE_ROOT
///   2. Strip known scope suffixes:   ("openai-backend", "base-url") → HARMONIA_OPENAI_BASE_URL
///   3. Apply stem aliases:           ("harmonic-matrix", "db") → HARMONIA_MATRIX_DB
///
/// The few entries that break all three rules carry an explicit `env_override`.

/// A known configuration entry.
pub(crate) struct Entry {
    pub scope: &'static str,
    pub key: &'static str,
    /// Explicit env var name. If `None`, derived by [`derive_env_name`].
    pub env_override: Option<&'static str>,
}

/// Suffixes stripped from scope names to produce concise env var names.
const SCOPE_SUFFIXES: &[&str] = &["-backend", "-frontend", "-tool", "-core", "-storage"];

/// Scope stems that get aliased after suffix stripping.
const STEM_ALIASES: &[(&str, &str)] = &[
    ("harmonic-matrix", "matrix"),
    ("search-exa", "exa"),
    ("search-brave", "brave"),
    ("amazon-bedrock", "bedrock"),
];

/// Derive the env var name for a (scope, key) pair using the three-rule system.
pub(crate) fn derive_env_name(scope: &str, key: &str) -> String {
    let key_upper = key.to_ascii_uppercase().replace('-', "_");

    // Rule 1: global scope drops its name entirely.
    if scope == "global" {
        return format!("HARMONIA_{key_upper}");
    }

    // Rule 2: strip the first matching suffix.
    let mut stem = scope.to_string();
    for suffix in SCOPE_SUFFIXES {
        if let Some(stripped) = scope.strip_suffix(suffix) {
            stem = stripped.to_string();
            break;
        }
    }

    // Rule 3: apply stem alias if one exists.
    for &(from, to) in STEM_ALIASES {
        if stem == from {
            stem = to.to_string();
            break;
        }
    }

    let stem_upper = stem.to_ascii_uppercase().replace('-', "_");
    format!("HARMONIA_{stem_upper}_{key_upper}")
}

/// Get the env var name for a (scope, key) pair.
/// Checks registry overrides first, then falls back to derivation.
pub(crate) fn env_name(scope: &str, key: &str) -> String {
    for entry in REGISTRY {
        if entry.scope.eq_ignore_ascii_case(scope) && entry.key.eq_ignore_ascii_case(key) {
            if let Some(name) = entry.env_override {
                return name.to_string();
            }
            return derive_env_name(entry.scope, entry.key);
        }
    }
    // Unknown entry — still derive a reasonable name.
    derive_env_name(scope, key)
}

/// All known (scope, key, env_name) triples, derived from the single registry.
pub(crate) fn all_entries() -> Vec<(&'static str, &'static str, String)> {
    REGISTRY
        .iter()
        .map(|e| {
            let env = match e.env_override {
                Some(name) => name.to_string(),
                None => derive_env_name(e.scope, e.key),
            };
            (e.scope, e.key, env)
        })
        .collect()
}

// ─── Registry ───────────────────────────────────────────────────────

pub(crate) const REGISTRY: &[Entry] = &[
    // ── global ──
    Entry {
        scope: "global",
        key: "state-root",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "source-dir",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "lib-dir",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "share-dir",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "data-dir",
        env_override: Some("HARMONIA_DATA_DIR"),
    },
    Entry {
        scope: "global",
        key: "run-dir",
        env_override: Some("HARMONIA_RUN_DIR"),
    },
    Entry {
        scope: "global",
        key: "log-dir",
        env_override: Some("HARMONIA_LOG_DIR"),
    },
    Entry {
        scope: "global",
        key: "wallet-root",
        env_override: Some("HARMONIA_WALLET_ROOT"),
    },
    Entry {
        scope: "global",
        key: "wallet-db",
        env_override: Some("HARMONIA_VAULT_WALLET_DB"),
    },
    Entry {
        scope: "global",
        key: "vault-db",
        env_override: Some("HARMONIA_VAULT_DB"),
    },
    Entry {
        scope: "global",
        key: "env",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "fs-root",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "metrics-db",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "recovery-log",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "system-dir",
        env_override: None,
    },
    Entry {
        scope: "global",
        key: "log-level",
        env_override: Some("HARMONIA_LOG_LEVEL"),
    },
    Entry {
        scope: "global",
        key: "hrmw-bin",
        env_override: Some("HARMONIA_HRMW_BIN"),
    },
    // ── node ──
    Entry {
        scope: "node",
        key: "label",
        env_override: Some("HARMONIA_NODE_LABEL"),
    },
    Entry {
        scope: "node",
        key: "role",
        env_override: Some("HARMONIA_NODE_ROLE"),
    },
    Entry {
        scope: "node",
        key: "install-profile",
        env_override: Some("HARMONIA_INSTALL_PROFILE"),
    },
    Entry {
        scope: "node",
        key: "pair-code",
        env_override: Some("HARMONIA_PAIR_CODE"),
    },
    // ── openai ──
    Entry {
        scope: "openai-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "openai-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "openai-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── anthropic ──
    Entry {
        scope: "anthropic-backend",
        key: "api-version",
        env_override: Some("HARMONIA_ANTHROPIC_VERSION"),
    },
    Entry {
        scope: "anthropic-backend",
        key: "max-tokens",
        env_override: None,
    },
    Entry {
        scope: "anthropic-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "anthropic-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── xai ──
    Entry {
        scope: "xai-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "xai-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "xai-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── groq ──
    Entry {
        scope: "groq-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "groq-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "groq-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── alibaba ──
    Entry {
        scope: "alibaba-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "alibaba-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "alibaba-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── google ai studio ──
    Entry {
        scope: "google-ai-studio-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "google-ai-studio-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "google-ai-studio-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── google vertex ──
    Entry {
        scope: "google-vertex-backend",
        key: "project-id",
        env_override: None,
    },
    Entry {
        scope: "google-vertex-backend",
        key: "location",
        env_override: None,
    },
    Entry {
        scope: "google-vertex-backend",
        key: "endpoint",
        env_override: None,
    },
    Entry {
        scope: "google-vertex-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "google-vertex-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── amazon bedrock ──
    Entry {
        scope: "amazon-bedrock-backend",
        key: "region",
        env_override: None,
    },
    // ── openrouter ──
    Entry {
        scope: "openrouter-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "openrouter-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── voice backends ──
    Entry {
        scope: "whisper-backend",
        key: "groq-api-url",
        env_override: None,
    },
    Entry {
        scope: "whisper-backend",
        key: "openai-api-url",
        env_override: None,
    },
    Entry {
        scope: "whisper-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "whisper-backend",
        key: "max-time-secs",
        env_override: None,
    },
    Entry {
        scope: "elevenlabs-backend",
        key: "base-url",
        env_override: None,
    },
    Entry {
        scope: "elevenlabs-backend",
        key: "connect-timeout-secs",
        env_override: None,
    },
    Entry {
        scope: "elevenlabs-backend",
        key: "max-time-secs",
        env_override: None,
    },
    // ── tools ──
    Entry {
        scope: "search-exa-tool",
        key: "api-url",
        env_override: None,
    },
    Entry {
        scope: "search-brave-tool",
        key: "api-url",
        env_override: None,
    },
    // ── frontends ──
    Entry {
        scope: "mqtt-frontend",
        key: "broker",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "timeout-ms",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "tls",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "ca-cert",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "client-cert",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "client-key",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "trusted-client-fingerprints-json",
        env_override: None,
    },
    Entry {
        scope: "mqtt-frontend",
        key: "trusted-device-registry-json",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "bind",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "ca-cert",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "server-cert",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "server-key",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "trusted-client-fingerprints-json",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "max-concurrent-streams",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "session-idle-timeout-ms",
        env_override: None,
    },
    Entry {
        scope: "http2-frontend",
        key: "max-frame-bytes",
        env_override: None,
    },
    Entry {
        scope: "email-frontend",
        key: "api-url",
        env_override: None,
    },
    Entry {
        scope: "email-frontend",
        key: "from",
        env_override: None,
    },
    Entry {
        scope: "email-frontend",
        key: "default-subject",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "bitcoin-asp-url",
        env_override: Some("HARMONIA_ARK_ASP_URL"),
    },
    Entry {
        scope: "payment-auth",
        key: "identity-mode",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "identity-price",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "identity-unit",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "identity-allowed-rails",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "post-mode",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "post-price",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "post-unit",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "post-allowed-rails",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "comment-mode",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "comment-price",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "comment-unit",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "comment-allowed-rails",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "rate-mode",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "rate-price",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "rate-unit",
        env_override: None,
    },
    Entry {
        scope: "payment-auth",
        key: "rate-allowed-rails",
        env_override: None,
    },
    Entry {
        scope: "push-frontend",
        key: "mode",
        env_override: None,
    },
    Entry {
        scope: "push-frontend",
        key: "log",
        env_override: None,
    },
    Entry {
        scope: "nostr-frontend",
        key: "api-url",
        env_override: None,
    },
    Entry {
        scope: "mattermost-frontend",
        key: "api-url",
        env_override: None,
    },
    Entry {
        scope: "whatsapp-frontend",
        key: "api-url",
        env_override: None,
    },
    Entry {
        scope: "imessage-frontend",
        key: "server-url",
        env_override: None,
    },
    Entry {
        scope: "discord-frontend",
        key: "channels",
        env_override: None,
    },
    Entry {
        scope: "slack-frontend",
        key: "channels",
        env_override: None,
    },
    Entry {
        scope: "signal-frontend",
        key: "rpc-url",
        env_override: None,
    },
    Entry {
        scope: "signal-frontend",
        key: "account",
        env_override: None,
    },
    // ── harmonic matrix ──
    Entry {
        scope: "harmonic-matrix",
        key: "store-kind",
        env_override: None,
    },
    Entry {
        scope: "harmonic-matrix",
        key: "db",
        env_override: None,
    },
    Entry {
        scope: "harmonic-matrix",
        key: "graph-uri",
        env_override: None,
    },
    Entry {
        scope: "harmonic-matrix",
        key: "history-limit",
        env_override: None,
    },
    Entry {
        scope: "harmonic-matrix",
        key: "route-signal-default",
        env_override: Some("HARMONIA_ROUTE_SIGNAL_DEFAULT"),
    },
    Entry {
        scope: "harmonic-matrix",
        key: "route-noise-default",
        env_override: Some("HARMONIA_ROUTE_NOISE_DEFAULT"),
    },
    Entry {
        scope: "harmonic-matrix",
        key: "topology-path",
        env_override: None,
    },
    // ── phoenix ──
    Entry {
        scope: "phoenix-core",
        key: "trauma-log",
        env_override: Some("PHOENIX_TRAUMA_LOG"),
    },
    Entry {
        scope: "phoenix-core",
        key: "child-cmd",
        env_override: Some("PHOENIX_CHILD_CMD"),
    },
    Entry {
        scope: "phoenix-core",
        key: "max-restarts",
        env_override: Some("PHOENIX_MAX_RESTARTS"),
    },
    Entry {
        scope: "phoenix-core",
        key: "allow-prod-genesis",
        env_override: Some("HARMONIA_ALLOW_PROD_GENESIS"),
    },
    // ── chronicle ──
    Entry {
        scope: "chronicle",
        key: "db",
        env_override: Some("HARMONIA_CHRONICLE_DB"),
    },
    // ── tailnet ──
    Entry {
        scope: "tailnet-core",
        key: "port",
        env_override: None,
    },
    Entry {
        scope: "tailnet-core",
        key: "advertise-addr",
        env_override: Some("HARMONIA_TAILNET_ADVERTISE_ADDR"),
    },
    Entry {
        scope: "tailnet-core",
        key: "advertise-host",
        env_override: Some("HARMONIA_TAILNET_ADVERTISE_HOST"),
    },
    Entry {
        scope: "tailnet-core",
        key: "hostname-prefix",
        env_override: None,
    },
    Entry {
        scope: "tailnet-core",
        key: "shared-secret",
        env_override: Some("HARMONIA_MESH_SHARED_SECRET"),
    },
    // ── tailscale integration (CLI) ──
    Entry {
        scope: "tailscale",
        key: "socket",
        env_override: Some("HARMONIA_TAILSCALE_SOCKET"),
    },
    Entry {
        scope: "tailscale",
        key: "localapi-port",
        env_override: Some("HARMONIA_TAILSCALE_LOCALAPI_PORT"),
    },
    // ── memory ──
    Entry {
        scope: "memory",
        key: "night-start",
        env_override: Some("HARMONIA_MEMORY_NIGHT_START"),
    },
    Entry {
        scope: "memory",
        key: "night-end",
        env_override: Some("HARMONIA_MEMORY_NIGHT_END"),
    },
    Entry {
        scope: "memory",
        key: "idle-seconds",
        env_override: Some("HARMONIA_MEMORY_IDLE_SECONDS"),
    },
    Entry {
        scope: "memory",
        key: "heartbeat-seconds",
        env_override: Some("HARMONIA_MEMORY_HEARTBEAT_SECONDS"),
    },
    Entry {
        scope: "memory",
        key: "user-tz-hours-west",
        env_override: Some("HARMONIA_USER_TZ_HOURS_WEST"),
    },
    // ── ouroboros ──
    Entry {
        scope: "ouroboros-core",
        key: "patch-dir",
        env_override: None,
    },
    // ── s3 ──
    Entry {
        scope: "s3-storage",
        key: "mode",
        env_override: None,
    },
    Entry {
        scope: "s3-storage",
        key: "local-root",
        env_override: None,
    },
    // ── evolution ──
    Entry {
        scope: "evolution",
        key: "mode",
        env_override: None,
    },
    Entry {
        scope: "evolution",
        key: "source-rewrite-enabled",
        env_override: Some("HARMONIA_SOURCE_REWRITE_ENABLED"),
    },
    Entry {
        scope: "evolution",
        key: "distributed-enabled",
        env_override: Some("HARMONIA_DISTRIBUTED_EVOLUTION_ENABLED"),
    },
    Entry {
        scope: "evolution",
        key: "distributed-store-kind",
        env_override: Some("HARMONIA_DISTRIBUTED_STORE_KIND"),
    },
    Entry {
        scope: "evolution",
        key: "distributed-store-bucket",
        env_override: Some("HARMONIA_DISTRIBUTED_STORE_BUCKET"),
    },
    Entry {
        scope: "evolution",
        key: "distributed-store-prefix",
        env_override: Some("HARMONIA_DISTRIBUTED_STORE_PREFIX"),
    },
    // ── policies / orchestration ──
    Entry {
        scope: "model-policy",
        key: "path",
        env_override: None,
    },
    Entry {
        scope: "model-policy",
        key: "planner",
        env_override: Some("HARMONIA_MODEL_PLANNER"),
    },
    Entry {
        scope: "model-policy",
        key: "planner-model",
        env_override: Some("HARMONIA_MODEL_PLANNER_MODEL"),
    },
    Entry {
        scope: "harmony-policy",
        key: "path",
        env_override: None,
    },
    Entry {
        scope: "parallel-agents-core",
        key: "policy-path",
        env_override: Some("HARMONIA_PARALLEL_POLICY_PATH"),
    },
    Entry {
        scope: "signalograd-core",
        key: "state-path",
        env_override: None,
    },
    // ── observability ──
    Entry {
        scope: "observability",
        key: "enabled",
        env_override: Some("HARMONIA_OBSERVABILITY_ENABLED"),
    },
    Entry {
        scope: "observability",
        key: "trace-level",
        env_override: Some("HARMONIA_OBSERVABILITY_TRACE_LEVEL"),
    },
    Entry {
        scope: "observability",
        key: "sample-rate",
        env_override: Some("HARMONIA_OBSERVABILITY_SAMPLE_RATE"),
    },
    Entry {
        scope: "observability",
        key: "project-name",
        env_override: Some("HARMONIA_OBSERVABILITY_PROJECT_NAME"),
    },
    Entry {
        scope: "observability",
        key: "api-url",
        env_override: Some("HARMONIA_OBSERVABILITY_API_URL"),
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_scope_drops_name() {
        assert_eq!(
            derive_env_name("global", "state-root"),
            "HARMONIA_STATE_ROOT"
        );
        assert_eq!(derive_env_name("global", "env"), "HARMONIA_ENV");
        assert_eq!(
            derive_env_name("global", "system-dir"),
            "HARMONIA_SYSTEM_DIR"
        );
    }

    #[test]
    fn suffix_stripping() {
        assert_eq!(
            derive_env_name("openai-backend", "base-url"),
            "HARMONIA_OPENAI_BASE_URL"
        );
        assert_eq!(
            derive_env_name("mqtt-frontend", "broker"),
            "HARMONIA_MQTT_BROKER"
        );
        assert_eq!(
            derive_env_name("whisper-backend", "groq-api-url"),
            "HARMONIA_WHISPER_GROQ_API_URL"
        );
        assert_eq!(
            derive_env_name("elevenlabs-backend", "base-url"),
            "HARMONIA_ELEVENLABS_BASE_URL"
        );
        assert_eq!(
            derive_env_name("tailnet-core", "port"),
            "HARMONIA_TAILNET_PORT"
        );
        assert_eq!(derive_env_name("s3-storage", "mode"), "HARMONIA_S3_MODE");
    }

    #[test]
    fn stem_aliases() {
        assert_eq!(
            derive_env_name("harmonic-matrix", "db"),
            "HARMONIA_MATRIX_DB"
        );
        assert_eq!(
            derive_env_name("harmonic-matrix", "store-kind"),
            "HARMONIA_MATRIX_STORE_KIND"
        );
        assert_eq!(
            derive_env_name("search-exa-tool", "api-url"),
            "HARMONIA_EXA_API_URL"
        );
        assert_eq!(
            derive_env_name("search-brave-tool", "api-url"),
            "HARMONIA_BRAVE_API_URL"
        );
        assert_eq!(
            derive_env_name("amazon-bedrock-backend", "region"),
            "HARMONIA_BEDROCK_REGION"
        );
    }

    #[test]
    fn explicit_overrides() {
        assert_eq!(
            env_name("anthropic-backend", "api-version"),
            "HARMONIA_ANTHROPIC_VERSION"
        );
        assert_eq!(
            env_name("evolution", "source-rewrite-enabled"),
            "HARMONIA_SOURCE_REWRITE_ENABLED"
        );
        assert_eq!(env_name("phoenix-core", "trauma-log"), "PHOENIX_TRAUMA_LOG");
        assert_eq!(
            env_name("phoenix-core", "allow-prod-genesis"),
            "HARMONIA_ALLOW_PROD_GENESIS"
        );
    }

    #[test]
    fn unknown_entries_still_derive() {
        // Entries not in the registry still get a reasonable env name.
        assert_eq!(env_name("custom-backend", "foo"), "HARMONIA_CUSTOM_FOO");
        assert_eq!(env_name("global", "new-key"), "HARMONIA_NEW_KEY");
    }

    /// Exhaustive check: every registry entry must produce the expected env var
    /// name. This is the authoritative backward-compatibility test — if this
    /// passes, all existing env vars continue to work.
    #[test]
    fn all_entries_match_historic_names() {
        let expected: &[(&str, &str, &str)] = &[
            // global
            ("global", "state-root", "HARMONIA_STATE_ROOT"),
            ("global", "source-dir", "HARMONIA_SOURCE_DIR"),
            ("global", "lib-dir", "HARMONIA_LIB_DIR"),
            ("global", "share-dir", "HARMONIA_SHARE_DIR"),
            ("global", "data-dir", "HARMONIA_DATA_DIR"),
            ("global", "run-dir", "HARMONIA_RUN_DIR"),
            ("global", "log-dir", "HARMONIA_LOG_DIR"),
            ("global", "wallet-root", "HARMONIA_WALLET_ROOT"),
            ("global", "wallet-db", "HARMONIA_VAULT_WALLET_DB"),
            ("global", "vault-db", "HARMONIA_VAULT_DB"),
            ("global", "env", "HARMONIA_ENV"),
            ("global", "fs-root", "HARMONIA_FS_ROOT"),
            ("global", "metrics-db", "HARMONIA_METRICS_DB"),
            ("global", "recovery-log", "HARMONIA_RECOVERY_LOG"),
            ("global", "system-dir", "HARMONIA_SYSTEM_DIR"),
            ("global", "log-level", "HARMONIA_LOG_LEVEL"),
            ("global", "hrmw-bin", "HARMONIA_HRMW_BIN"),
            // node
            ("node", "label", "HARMONIA_NODE_LABEL"),
            ("node", "role", "HARMONIA_NODE_ROLE"),
            ("node", "install-profile", "HARMONIA_INSTALL_PROFILE"),
            ("node", "pair-code", "HARMONIA_PAIR_CODE"),
            // openai
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
            // anthropic
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
            // xai
            ("xai-backend", "base-url", "HARMONIA_XAI_BASE_URL"),
            (
                "xai-backend",
                "connect-timeout-secs",
                "HARMONIA_XAI_CONNECT_TIMEOUT_SECS",
            ),
            ("xai-backend", "max-time-secs", "HARMONIA_XAI_MAX_TIME_SECS"),
            // groq
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
            // alibaba
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
            // google ai studio
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
            // google vertex
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
            // amazon bedrock
            (
                "amazon-bedrock-backend",
                "region",
                "HARMONIA_BEDROCK_REGION",
            ),
            // openrouter
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
            // voice backends
            (
                "whisper-backend",
                "groq-api-url",
                "HARMONIA_WHISPER_GROQ_API_URL",
            ),
            (
                "whisper-backend",
                "openai-api-url",
                "HARMONIA_WHISPER_OPENAI_API_URL",
            ),
            (
                "whisper-backend",
                "connect-timeout-secs",
                "HARMONIA_WHISPER_CONNECT_TIMEOUT_SECS",
            ),
            (
                "whisper-backend",
                "max-time-secs",
                "HARMONIA_WHISPER_MAX_TIME_SECS",
            ),
            (
                "elevenlabs-backend",
                "base-url",
                "HARMONIA_ELEVENLABS_BASE_URL",
            ),
            (
                "elevenlabs-backend",
                "connect-timeout-secs",
                "HARMONIA_ELEVENLABS_CONNECT_TIMEOUT_SECS",
            ),
            (
                "elevenlabs-backend",
                "max-time-secs",
                "HARMONIA_ELEVENLABS_MAX_TIME_SECS",
            ),
            // tools
            ("search-exa-tool", "api-url", "HARMONIA_EXA_API_URL"),
            ("search-brave-tool", "api-url", "HARMONIA_BRAVE_API_URL"),
            // frontends
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
            // harmonic matrix
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
            // phoenix
            ("phoenix-core", "trauma-log", "PHOENIX_TRAUMA_LOG"),
            ("phoenix-core", "child-cmd", "PHOENIX_CHILD_CMD"),
            ("phoenix-core", "max-restarts", "PHOENIX_MAX_RESTARTS"),
            (
                "phoenix-core",
                "allow-prod-genesis",
                "HARMONIA_ALLOW_PROD_GENESIS",
            ),
            // chronicle
            ("chronicle", "db", "HARMONIA_CHRONICLE_DB"),
            // tailnet
            ("tailnet-core", "port", "HARMONIA_TAILNET_PORT"),
            (
                "tailnet-core",
                "advertise-addr",
                "HARMONIA_TAILNET_ADVERTISE_ADDR",
            ),
            (
                "tailnet-core",
                "advertise-host",
                "HARMONIA_TAILNET_ADVERTISE_HOST",
            ),
            (
                "tailnet-core",
                "hostname-prefix",
                "HARMONIA_TAILNET_HOSTNAME_PREFIX",
            ),
            (
                "tailnet-core",
                "shared-secret",
                "HARMONIA_MESH_SHARED_SECRET",
            ),
            // tailscale integration
            (
                "tailscale",
                "socket",
                "HARMONIA_TAILSCALE_SOCKET",
            ),
            (
                "tailscale",
                "localapi-port",
                "HARMONIA_TAILSCALE_LOCALAPI_PORT",
            ),
            // memory
            ("memory", "night-start", "HARMONIA_MEMORY_NIGHT_START"),
            ("memory", "night-end", "HARMONIA_MEMORY_NIGHT_END"),
            ("memory", "idle-seconds", "HARMONIA_MEMORY_IDLE_SECONDS"),
            (
                "memory",
                "heartbeat-seconds",
                "HARMONIA_MEMORY_HEARTBEAT_SECONDS",
            ),
            (
                "memory",
                "user-tz-hours-west",
                "HARMONIA_USER_TZ_HOURS_WEST",
            ),
            // ouroboros
            (
                "ouroboros-core",
                "patch-dir",
                "HARMONIA_OUROBOROS_PATCH_DIR",
            ),
            // s3
            ("s3-storage", "mode", "HARMONIA_S3_MODE"),
            ("s3-storage", "local-root", "HARMONIA_S3_LOCAL_ROOT"),
            // evolution
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
            // policies
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
                "signalograd-core",
                "state-path",
                "HARMONIA_SIGNALOGRAD_STATE_PATH",
            ),
            // observability
            (
                "observability",
                "enabled",
                "HARMONIA_OBSERVABILITY_ENABLED",
            ),
            (
                "observability",
                "trace-level",
                "HARMONIA_OBSERVABILITY_TRACE_LEVEL",
            ),
            (
                "observability",
                "sample-rate",
                "HARMONIA_OBSERVABILITY_SAMPLE_RATE",
            ),
            (
                "observability",
                "project-name",
                "HARMONIA_OBSERVABILITY_PROJECT_NAME",
            ),
            (
                "observability",
                "api-url",
                "HARMONIA_OBSERVABILITY_API_URL",
            ),
        ];

        let actual = all_entries();
        assert_eq!(
            actual.len(),
            expected.len(),
            "registry has {} entries but expected {}",
            actual.len(),
            expected.len()
        );

        for (scope, key, want) in expected {
            let got = env_name(scope, key);
            assert_eq!(
                got, *want,
                "mismatch for ({scope}, {key}): got {got}, want {want}"
            );
        }
    }

    #[test]
    fn no_duplicate_entries() {
        let mut seen = std::collections::HashSet::new();
        for entry in REGISTRY {
            assert!(
                seen.insert((entry.scope, entry.key)),
                "duplicate registry entry: ({}, {})",
                entry.scope,
                entry.key
            );
        }
    }

    #[test]
    fn route_entries_use_derivation_correctly() {
        // These are the trickiest: harmonic-matrix entries where the stem alias
        // is applied but the env name drops "matrix" for route-* keys.
        // They use the stem alias "matrix" which produces HARMONIA_MATRIX_ROUTE_*
        // but historically they were HARMONIA_ROUTE_*. Verify overrides are set
        // where needed or derivation matches.
        assert_eq!(
            env_name("harmonic-matrix", "route-signal-default"),
            "HARMONIA_ROUTE_SIGNAL_DEFAULT"
        );
        assert_eq!(
            env_name("harmonic-matrix", "route-noise-default"),
            "HARMONIA_ROUTE_NOISE_DEFAULT"
        );
    }
}
