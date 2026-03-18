use super::Entry;

pub(crate) const ENTRIES: &[Entry] = &[
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
];
