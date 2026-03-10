# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-03-10

### Added
- **Config-store v2**: SQLite-backed scoped configuration store with in-memory cache, component access policies, and env var fallback chain
  - Policy engine mirrors vault's component-scoped model (read/write/delete per component)
  - Admin components (`conductor`, `admin-intent`, `harmonia-cli`) get full access
  - Fallback chain: cache → DB → legacy env alias → canonical `HARMONIA_{SCOPE}_{KEY}` → default
  - Legacy env var seeding on first init (captures existing env vars permanently)
  - 22 config-store unit tests
- **9 native LLM backend crates** with standardised provider-protocol:
  - `harmonia-openai`, `harmonia-anthropic`, `harmonia-xai`, `harmonia-groq`, `harmonia-alibaba`
  - `harmonia-google-ai-studio`, `harmonia-google-vertex`, `harmonia-amazon-bedrock`
  - `harmonia-provider-protocol` (shared model pool, metrics, FFI)
- **Provider protocol model pool**: hardcoded offerings per backend with automatic model selection, fallback chains, and performance tracking via `metrics.db`
- `harmonia-discord` frontend crate
- `harmonia-signal` frontend crate
- Lisp CFFI bindings for config-store v2 (`config-get-for`, `config-get-or`, `config-set-for`, `config-delete-for`, `config-dump`)
- Vault component policy expanded for frontend legacy migration paths
- Parallel-agents FFI layer (`harmonia_parallel_agents_*`)

### Changed
- **Eliminated ~53 raw `env::var()` calls** across 30+ crates — all non-secret config now flows through config-store
- **Setup wizard restructured**: required steps (workspace, SBCL, vault/config-store, LLM provider) separated from optional (frontends, tools, git, S3, evolution)
- Setup writes directly to config-store and vault — no more `runtime.env` file generation
- `cli/start.rs` passes only 4 bootstrap env vars to SBCL subprocess (was 8+)
- 7 Lisp files migrated from `sb-ext:posix-getenv` to `config-get-for`/`config-get-or`
- Frontend secrets exclusively via vault (no env var fallback); non-secret config via config-store
- OpenRouter backend rewritten with expanded model catalogue and background API sync
- `harmonia-config-store` bumped to 0.2.0
- Root crate bumped to 0.2.0

### Removed
- `runtime.env` file generation and loading
- `lib/backends/llms/openrouter/src/state.rs` (replaced by provider-protocol metrics)
- Direct env var reads for non-bootstrap, non-secret configuration

## [0.1.0] — 2026-03-05

### Added
- Initial open-source release
- 35 Rust crates organized in four pillars: core (17), backends (1), tools (6), frontends (10)
- Common Lisp orchestration layer (SBCL) with self-rewriting engine
- Gateway signal baseband processor with hot-pluggable frontend .so loading
- Tailscale mesh networking for inter-node communication
- Browser tool v2.0 with 3-layer security, Chrome CDP support, SSRF-safe controlled fetch
- 11 extraction macros including audio source extraction
- 7 frontend channels: TUI, MQTT, iMessage, WhatsApp, Telegram, Slack, Tailscale
- Interactive setup wizard (`harmonia setup`)
- Cross-platform install script (macOS, Linux, FreeBSD, Windows)
- Vault-based secret management (SQLite)
- Harmonic matrix routing and scoring
- Parallel agent orchestration (tmux-based swarm)
- Self-rewrite protocol (Claude Code > Codex > OpenRouter)
- 8 Laws of Harmonia evolution framework
