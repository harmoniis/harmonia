# Harmonia Agent — Agent Skill

Binary: `harmonia` | Install: `curl --proto '=https' --tlsv1.2 -sSf https://harmoniis.com/harmonia/install | sh` | Windows: `iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex`

Distributed evolutionary homoiconic self-improving agent. SBCL Common Lisp core + modular Rust tool ecosystem via CFFI.

## Install

macOS / Linux / FreeBSD:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://harmoniis.com/harmonia/install | sh
```

Windows (PowerShell):
```powershell
iwr https://harmoniis.com/harmonia/install.ps1 -UseB | iex
```

Binary-first install with source-build fallback. Optional overrides:
```bash
HARMONIA_INSTALL_MODE=source curl -sSf https://harmoniis.com/harmonia/install | sh   # force source build
HARMONIA_WITH_SOURCE=1 curl -sSf https://harmoniis.com/harmonia/install | sh          # binary + local source for self-rewrite
```

### Prerequisites

| Dependency | Why | Install |
|---|---|---|
| Rust 1.75+ | Builds tool crates | rustup.rs |
| SBCL | Agent runtime | `brew install sbcl` / `apt install sbcl` |
| Quicklisp | Lisp packages | Auto-installed by `harmonia setup` |

## Setup

```bash
harmonia setup    # interactive wizard (10 steps)
harmonia start    # start agent
harmonia upgrade  # update to latest release
harmonia uninstall
```

### Setup Wizard Steps

1. Creates system workspace at `~/.harmoniis/harmonia/`
2. Asks for user workspace directory (default: `~/workspace`)
3. Verifies SBCL + Quicklisp
4. Bootstraps vault keys from `harmoniis-wallet`
5. Select LLM providers: OpenRouter, OpenAI, Anthropic, xAI, Google, Vertex, Bedrock/Nova, Groq, Alibaba
6. Choose evolution profile: `binary-only` | `source-rewrite` | `distributed participant`
7. Stores credentials in encrypted vault with component-scoped policies
8. Select frontends to enable
9. Collect frontend credentials (API tokens, URLs)
10. Build and configure everything

## Configuration

Workspace: `~/.harmoniis/harmonia/`

```
~/.harmoniis/harmonia/
├── vault.db          # encrypted secrets (SQLite)
├── config/
│   ├── workspace.sexp
│   ├── gateway-frontends.sexp
│   └── runtime.env   # LLM runtime defaults
├── genesis/          # evolution knowledge
└── frontends/        # compiled .so libraries
```

Key config files (S-expressions):
- `harmony-policy.sexp` — security, evolution, harmonic routing
- `model-policy.sexp` — LLM profiles, task routing, pricing weights
- `baseband.sexp` — frontend registry + mesh transport
- `tools.sexp` — tool availability
- `swarm.sexp` — parallel agent orchestration

## Architecture

```
SBCL Common Lisp (Conductor + Memory + Self-Rewrite/Ouroboros)
         │ CFFI
Rust .so Libraries (Core + Tools + Backend + Frontends)
```

### Core Crates
phoenix (hot-reload/recovery), ouroboros (self-rewrite), vault (encrypted secrets), memory (persistent + compression), http, s3, git-ops, rust-forge (runtime compilation), cron-scheduler, push (webhooks), recovery, fs, parallel-agents, harmonic-matrix, config-store, tailnet, gateway.

### Backend
openrouter-backend — multi-provider LLM router (OpenRouter, OpenAI, Anthropic, xAI, Google/Vertex, Bedrock/Nova, Groq, Alibaba).

### Tools
browser (Chrome CDP, sandboxed), search-exa (neural), search-brave, whisper (STT), elevenlabs (TTS), social.

### Frontends (hot-pluggable via dlopen)
TUI (always on), MQTT, Telegram, Slack, WhatsApp, iMessage (BlueBubbles), Mattermost, Nostr, Email (IMAP/SMTP), Tailscale mesh.

## Security

Two-layer model:
1. **Security Kernel** (deterministic, non-bypassable) — typed signal dispatch, policy gate for privileged ops, taint propagation, safe parsers.
2. **Adaptive Security Shell** — dissonance scoring (18 injection patterns), harmonic routing, component-scoped vault policies.

Policy gate blocks privileged ops if signal has external taint or lacks `:owner`/`:authenticated` label.

## Evolution Profiles

- `binary-only` — use released binaries, no self-modification
- `source-rewrite` — agent can edit and recompile its own source
- `distributed participant` — participate in cross-instance evolution

## LLM Model Routing

13 profiles from micro (nova-micro) to frontier (claude-opus-4.6, gpt-5). Task-specific routing: software-dev, reasoning, memory-ops, etc. Scoring weights: completion 30%, correctness 20%, speed 12%, price 12%.

## Platforms

macOS (Apple Silicon / Intel), Linux (x86_64 / aarch64), FreeBSD, Windows.

## License

BSL 1.1. Free for personal non-revenue use. Production use: 3% of prior-year revenue. Buy at https://harmoniis.com/pricing/subscriptions. Converts to Apache 2.0 on 2030-03-05.

## Build from Source

```bash
git clone https://github.com/harmoniis/harmonia.git && cd harmonia
bazel build //...   # or: cargo build --release
bazel test //...    # or: cargo test --workspace
```

Source: https://github.com/harmoniis/harmonia
Docs: https://harmoniis.com/docs/harmonia/overview
Marketplace: https://harmoniis.com/skill.md
