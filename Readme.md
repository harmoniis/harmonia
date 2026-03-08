<p align="center">
  <strong>Harmonia</strong><br>
  <em>Self-improving Common Lisp + Rust agent</em>
</p>

<p align="center">
  <a href="https://crates.io/crates/harmonia"><img src="https://img.shields.io/crates/v/harmonia.svg" alt="crates.io"></a>
  <a href="https://github.com/harmoniis/harmonia/actions"><img src="https://github.com/harmoniis/harmonia/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-BUSL--1.1-orange.svg" alt="Business Source License 1.1"></a>
</p>

---

Harmonia is a recursive self-improving agent built on SBCL Common Lisp with a modular Rust tool ecosystem. The Lisp core handles orchestration, reasoning, memory, and self-rewriting. All I/O — messaging, search, storage, LLM calls — flows through hot-pluggable Rust `.so` libraries loaded via CFFI.

## Install

**macOS / Linux / FreeBSD / NetBSD:**

```bash
curl -sSf https://harmoniis.com/harmonia/install | sh
```

By default this installer is **binary-first** (GitHub release artifacts) with source-build fallback.

Optional install mode overrides:

```bash
# Force source build install
HARMONIA_INSTALL_MODE=source curl -sSf https://harmoniis.com/harmonia/install | sh

# Binary install + optional local source checkout for source-rewrite workflows
HARMONIA_WITH_SOURCE=1 curl -sSf https://harmoniis.com/harmonia/install | sh
```

**Windows (PowerShell):**

```powershell
iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex
```

**From source:**

```bash
cargo install harmonia
harmonia setup
```

### Prerequisites

| Dependency | Why | Install |
|------------|-----|---------|
| **Rust** 1.75+ | Builds all tool crates | [rustup.rs](https://rustup.rs) |
| **SBCL** | Agent runtime | `brew install sbcl` / `apt install sbcl` |
| **Quicklisp** | Lisp package manager | Auto-installed by `harmonia setup` |

## Quick start

```bash
# Interactive setup — configures workspace, API keys, frontends
harmonia setup

# Start the agent
harmonia start

# Start in production mode
harmonia start --env prod
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

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  SBCL Common Lisp                │
│  ┌───────────┐ ┌──────────┐ ┌────────────────┐  │
│  │ Conductor  │ │  Memory  │ │  Self-Rewrite  │  │
│  │ (Planner)  │ │ (Store)  │ │  (Ouroboros)   │  │
│  └─────┬─────┘ └────┬─────┘ └───────┬────────┘  │
│        └─────────────┴───────────────┘           │
│                      │ CFFI                      │
├──────────────────────┼───────────────────────────┤
│                Rust .so Libraries                │
│  ┌──────┐ ┌───────┐ ┌────────┐ ┌─────────────┐  │
│  │ Core │ │ Tools │ │Backend │ │  Frontends   │  │
│  └──────┘ └───────┘ └────────┘ └─────────────┘  │
└─────────────────────────────────────────────────┘
```

**Core** — vault, memory, HTTP, S3, git, cron, recovery, filesystem, gateway, mesh networking

**Tools** — browser (Chrome CDP), web search (Exa/Brave), speech (Whisper/ElevenLabs), social

**Backends** — multi-provider LLM routing (OpenRouter + native provider adapters)

**Frontends** — hot-pluggable messaging channels loaded at runtime via `dlopen`

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
| `harmonia-config-store` | Configuration management |
| `harmonia-tailnet` | Tailscale mesh networking |
| `harmonia-gateway` | Frontend gateway and signal routing |

### Backends

| Crate | Description |
|-------|-------------|
| `harmonia-openrouter-backend` | Multi-provider LLM router (OpenRouter, OpenAI, Anthropic, xAI, Google AI Studio/Vertex, Bedrock/Nova, Groq, Alibaba) |

### Tools

| Crate | Description |
|-------|-------------|
| `harmonia-browser` | Headless browser with Chrome CDP support |
| `harmonia-search-exa` | Exa neural search |
| `harmonia-search-brave` | Brave web search |
| `harmonia-whisper` | Speech-to-text via Whisper |
| `harmonia-elevenlabs` | Text-to-speech via ElevenLabs |
| `harmonia-social` | Social media integrations |

### Frontends

| Crate | Description |
|-------|-------------|
| `harmonia-tui` | Terminal UI (always enabled) |
| `harmonia-mqtt-client` | MQTT messaging |
| `harmonia-telegram` | Telegram bot |
| `harmonia-slack` | Slack bot |
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

After running `harmonia setup`, the system workspace lives at `~/.harmoniis/harmonia/`:

```
~/.harmoniis/harmonia/
├── vault.db          # Encrypted secrets (SQLite)
├── config/           # Runtime configuration (S-expressions)
│   ├── workspace.sexp
│   ├── gateway-frontends.sexp
│   └── runtime.env   # Setup-generated LLM runtime defaults/fallbacks
├── genesis/          # Agent evolution knowledge
└── frontends/        # Compiled frontend libraries
```

## Supported platforms

| Platform | Install | Status |
|----------|---------|--------|
| macOS (Apple Silicon / Intel) | `curl -sSf ... \| sh` | Supported |
| Linux (x86_64 / aarch64) | `curl -sSf ... \| sh` | Supported |
| FreeBSD | `curl -sSf ... \| sh` | Supported |
| NetBSD | `curl -sSf ... \| sh` | Supported |
| Windows | `iwr ... \| iex` | Supported |

## License

[Business Source License 1.1 (BUSL-1.1)](LICENSE) with an Additional Use Grant.
Commercial production use outside that grant requires a separate commercial
license from Harmoniis Agents Ltd. The Change Date for this version is
**2030-03-05**, after which it converts to **Apache License, Version 2.0**.
