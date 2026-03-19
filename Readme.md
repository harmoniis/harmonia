<p align="center">
<pre>
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
</pre>
</p>

<p align="center">
  <em>Distributed evolutionary homoiconic self-improving agent</em>
</p>

<p align="center">
  <a href="https://github.com/harmoniis/harmonia/actions"><img src="https://github.com/harmoniis/harmonia/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-BUSL--1.1-orange.svg" alt="Business Source License 1.1"></a>
</p>

---

Harmonia is a recursive self-improving agent built on SBCL Common Lisp with a modular Rust tool ecosystem. The Lisp core handles orchestration, reasoning, memory, and self-rewriting. All I/O — messaging, search, storage, LLM calls — flows through `harmonia-runtime`, a single Rust binary containing all ractor actors, communicating with SBCL via IPC over a Unix domain socket.

## Install

**macOS / Linux / FreeBSD:**

```bash
curl -sSf https://harmoniis.com/harmonia/install | sh
```

**Windows (PowerShell):**

```powershell
iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex
```

By default the installer is **binary-first** (GitHub release artifacts) with source-build fallback.

Optional install mode overrides:

```bash
# Force source build install
HARMONIA_INSTALL_MODE=source curl -sSf https://harmoniis.com/harmonia/install | sh

# Binary install + optional local source checkout for source-rewrite workflows
HARMONIA_WITH_SOURCE=1 curl -sSf https://harmoniis.com/harmonia/install | sh
```

### Prerequisites

| Dependency | Why | Install |
|------------|-----|---------|
| **Rust** 1.89+ | Builds all tool crates | [rustup.rs](https://rustup.rs) |
| **SBCL** | Agent runtime | `brew install sbcl` / `apt install sbcl` |
| **Quicklisp** | Lisp package manager | Auto-installed by `harmonia setup` |

## Quick start

```bash
# Interactive setup — configures workspace, API keys, frontends
harmonia setup

# Start the agent
harmonia start
```

The setup wizard will:

1. Create the system workspace at `~/.harmoniis/harmonia/`
2. Ask for your user workspace directory (default: `~/workspace`)
3. Verify SBCL and Quicklisp are installed
4. Bootstrap wallet-rooted vault keys from `harmoniis-wallet`
5. Let you select one or more LLM providers (OpenRouter, OpenAI, Anthropic, xAI, Google, Vertex, Bedrock/Nova, Groq, Alibaba)
6. Choose evolution profile (`binary-only`, `source-rewrite`, or `distributed participant`)
7. Store provider credentials in vault with component-scoped policies
8. Let you select which frontends to enable
9. Collect credentials for selected frontends
10. Build and configure everything

## Upgrade

```bash
harmonia upgrade
```

## Uninstall

```bash
harmonia uninstall
```

## Architecture

```
Phoenix (process supervisor, writes phoenix.pid)
  |
  |-- harmonia-runtime (single Rust binary, all ractor actors)
  |     |-- RuntimeSupervisor (actor registry, message routing)
  |     |-- SbclBridgeActor (Unix socket <-> SBCL)
  |     +-- IPC listener (runtime.sock)
  |
  |-- sbcl-agent (Lisp orchestrator)
  |     |-- Conductor (planner)
  |     |-- Memory (store)
  |     +-- Self-Rewrite (Ouroboros)
  |
  +-- provision-server
```

**Core** — vault, memory, HTTP, S3, git, cron, recovery, filesystem, gateway, mesh networking

**Tools** — browser (Chrome CDP with stealth engine), web search (Exa/Brave), hfetch (secure HTTP), zoom (meeting automation)

**Backends** — multi-provider LLM routing (OpenRouter + native provider adapters), voice routing (STT via Whisper, TTS via ElevenLabs)

**Frontends** — rlib crates compiled into `harmonia-runtime`

## Crate map

### Core

| Crate | Description |
|-------|-------------|
| `harmonia-phoenix` | Hot-reload and crash recovery |
| `harmonia-ouroboros` | Self-rewriting engine |
| `harmonia-vault` | Encrypted secret storage (SQLite) |
| `harmonia-memory` | Persistent memory with compression |
| `harmonia-http` | HTTP client with retry and rate limiting |
| `harmonia-s3` | S3-compatible object storage sync |
| `harmonia-git-ops` | Git operations (commit, push, branch) |
| `harmonia-rust-forge` | Runtime Rust code compilation |
| `harmonia-cron-scheduler` | Cron-style task scheduling |
| `harmonia-push` | HTTP webhook push notifications (rlib, consumed by mqtt-client) |
| `harmonia-recovery` | Crash recovery and state restoration |
| `harmonia-fs` | Filesystem operations |
| `harmonia-parallel-agents` | Parallel agent orchestration |
| `harmonia-harmonic-matrix` | Harmonic scoring and evolution |
| `harmonia-config-store` | SQLite-backed scoped configuration store (policy-gated, cached) |
| `harmonia-tailnet` | Tailscale mesh networking |
| `harmonia-gateway` | Frontend gateway and signal routing |

### Backends

| Crate | Description |
|-------|-------------|
| `harmonia-provider-protocol` | Shared model pool protocol, metrics, and helpers |
| `harmonia-openrouter` | Universal LLM gateway via OpenRouter |
| `harmonia-openai` | OpenAI native backend |
| `harmonia-anthropic` | Anthropic Messages API backend |
| `harmonia-xai` | xAI / Grok backend |
| `harmonia-google-ai-studio` | Google AI Studio (Gemini) backend |
| `harmonia-google-vertex` | Google Vertex AI backend |
| `harmonia-amazon-bedrock` | Amazon Bedrock / Nova backend |
| `harmonia-groq` | Groq backend |
| `harmonia-alibaba` | Alibaba / DashScope (Qwen) backend |
| `harmonia-voice-protocol` | Shared voice provider protocol and HTTP helpers |
| `harmonia-voice-router` | Voice backend routing (STT + TTS provider dispatch) |
| `harmonia-whisper` | Speech-to-text via Whisper (Groq primary, OpenAI fallback) |
| `harmonia-elevenlabs` | Text-to-speech via ElevenLabs |

### Tools

| Crate | Description |
|-------|-------------|
| `harmonia-browser` | Headless browser with Chrome CDP and stealth anti-detection |
| `harmonia-search-exa` | Exa neural search |
| `harmonia-search-brave` | Brave web search |
| `harmonia-hfetch` | Secure HTTP client with SSRF protection and injection detection |
| `harmonia-zoom` | Zoom meeting automation via browser |

### Frontends

| Crate | Description |
|-------|-------------|
| `harmonia-tui` | Terminal UI (always enabled) |
| `harmonia-mqtt-client` | MQTT messaging |
| `harmonia-telegram` | Telegram bot |
| `harmonia-slack` | Slack bot |
| `harmonia-discord` | Discord bot |
| `harmonia-signal` | Signal bridge frontend |
| `harmonia-whatsapp` | WhatsApp via bridge API |
| `harmonia-imessage` | iMessage via BlueBubbles |
| `harmonia-mattermost` | Mattermost bot |
| `harmonia-nostr` | Nostr protocol |
| `harmonia-email-client` | Email via IMAP/SMTP |
| `harmonia-tailscale-frontend` | Tailscale mesh frontend |

## Building from source

```bash
git clone https://github.com/harmoniis/harmonia.git
cd harmonia

# Build everything (Bazel)
bazel build //...

# Run all tests
bazel test //...

# Run the CLI
bazel run //harmonia:harmonia -- version

# Publish all crates to crates.io
bazel run //harmonia:publish

# Or use Cargo directly
cargo build --release
cargo test --workspace
cargo fmt --check
```

## Configuration

After running `harmonia setup`, the system workspace lives at `~/.harmoniis/harmonia/` and holds user data only:

```
~/.harmoniis/harmonia/
├── vault.db          # Encrypted secrets (SQLite, AES-256-GCM)
├── config.db         # Non-secret configuration (SQLite, config-store)
├── metrics.db        # Model performance metrics (SQLite, auto-created)
├── config/           # Runtime configuration (S-expressions)
│   ├── workspace.sexp
│   └── gateway-frontends.sexp
├── genesis/          # Agent evolution knowledge
├── state/            # Runtime state, sockets, caches
└── frontends/        # Frontend state and attachments
```

Installed application assets live outside the workspace:

- macOS / Linux / FreeBSD: `~/.local/bin/harmonia`, `~/.local/lib/harmonia/`, `~/.local/share/harmonia/`
- Windows: `%LOCALAPPDATA%\Harmonia\bin\`, `%LOCALAPPDATA%\Harmonia\lib\`, `%LOCALAPPDATA%\Harmonia\share\`

All non-secret configuration (URLs, timeouts, paths, modes, feature flags) is managed by `config-store` with component-scoped access policies. All secrets (API keys, tokens, passwords) are stored in `vault`. No raw environment variables are used at runtime except for bootstrap paths (`HARMONIA_STATE_ROOT`, `HARMONIA_LIB_DIR`).

## Supported platforms

| Platform | Install | Status |
|----------|---------|--------|
| macOS (Apple Silicon / Intel) | `curl -sSf ... \| sh` | Supported |
| Linux (x86_64 / aarch64) | `curl -sSf ... \| sh` | Supported |
| FreeBSD | `curl -sSf ... \| sh` | Supported |
| Windows | `iwr ... \| iex` | Supported |

## License

[Business Source License 1.1 (BUSL-1.1)](LICENSE).

- **Free** for personal, non-revenue use.
- **Production License** required for freelancers, organisations, and any revenue-generating use: **3% of prior-year annual gross revenue** for unlimited use per calendar year (prorated mid-year).
- Buy instantly at [harmoniis.com/pricing/subscriptions](https://harmoniis.com/pricing/subscriptions).

Change Date: **2030-03-05** → converts to **Apache License, Version 2.0**.
