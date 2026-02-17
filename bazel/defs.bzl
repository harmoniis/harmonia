"""Shared Bazel definitions for Harmonia lib."""

# Core — essential infrastructure the agent cannot function without
HARMONIA_LIB_CORE = [
    "phoenix",       # Supervisor (Rust binary, PID 1)
    "ouroboros",     # Self-healing / reflection cycle
    "vault",         # Zero-knowledge secret injection
    "memory",        # Vector/Graph database core
    "mqtt-client",   # MQTT signaling
    "http",          # HTTP client (with Vault injection)
    "s3-sync",       # S3 bulk storage (images/videos/backups)
    "git-ops",       # Git self-versioning
    "rust-forge",    # Compile Rust source -> .so at runtime
    "cron-scheduler",# Cron / heartbeat scheduling
    "push-sns",      # Push notifications (APNs/FCM via SNS)
    "recovery",      # Watchdog & crash handling
    "browser",       # Headless browser
    "fs",            # Sandboxed filesystem I/O
]

# Backends — LLM providers
HARMONIA_LIB_BACKENDS = ["openrouter-backend"]

# Tools — optional plugins
HARMONIA_LIB_TOOLS = [
    "pgp-identity",
    "webcash-wallet",
    "social",        # WhatsApp/Telegram/Discord
]
