"""Shared Bazel definitions for Harmonia lib — Four Pillars."""

# Core — essential infrastructure the agent cannot function without
HARMONIA_LIB_CORE = [
    "phoenix",          # Supervisor (Rust binary, PID 1)
    "ouroboros",        # Self-healing / reflection cycle
    "vault",            # Zero-knowledge secret injection
    "memory",           # Vector/Graph database core
    "git-ops",          # Git self-versioning
    "rust-forge",       # Compile Rust source -> .so at runtime
    "cron-scheduler",   # Cron / heartbeat scheduling
    "recovery",         # Watchdog & crash handling
    "fs",               # Sandboxed filesystem I/O
    "parallel-agents",  # Parallel agent orchestration
    "harmonic-matrix",  # Harmonic scoring and evolution
    "config-store",     # Configuration management
    "tailnet",          # Tailscale mesh networking
    "gateway",          # Frontend gateway and signal routing
]

# Backends — providers and adapters
HARMONIA_LIB_BACKENDS = [
    "http",              # HTTP client (with Vault injection)
    "s3",           # S3 bulk storage (images/videos/backups)
    "openrouter-backend", # LLM completion via OpenRouter
]

# Tools — utility plugins
HARMONIA_LIB_TOOLS = [
    "browser",       # Headless browser with Chrome CDP
    "search-exa",    # Exa neural search
    "search-brave",  # Brave web search
    "whisper",       # Speech-to-text
    "elevenlabs",    # Text-to-speech
    "social",        # Social media integrations
]

# Frontends — hot-pluggable communication channels
HARMONIA_LIB_FRONTENDS = [
    "tui",           # Terminal UI (always enabled)
    "push",          # HTTP webhook push (rlib, consumed by mqtt-client)
    "mqtt-client",   # MQTT messaging
    "telegram",      # Telegram bot
    "slack",         # Slack bot
    "whatsapp",      # WhatsApp via bridge API
    "imessage",      # iMessage via BlueBubbles
    "mattermost",    # Mattermost bot
    "nostr",         # Nostr protocol
    "email-client",  # Email via IMAP/SMTP
    "tailscale",     # Tailscale mesh frontend
]
