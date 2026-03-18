use super::Entry;

pub(crate) const ENTRIES: &[Entry] = &[
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
    Entry {
        scope: "chronicle",
        key: "db",
        env_override: Some("HARMONIA_CHRONICLE_DB"),
    },
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
    Entry {
        scope: "ouroboros-core",
        key: "patch-dir",
        env_override: None,
    },
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
