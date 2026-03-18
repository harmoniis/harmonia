use super::Entry;

pub(crate) const ENTRIES: &[Entry] = &[
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
];
