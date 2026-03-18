use super::Entry;

pub(crate) const ENTRIES: &[Entry] = &[
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
    Entry {
        scope: "amazon-bedrock-backend",
        key: "region",
        env_override: None,
    },
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
];
