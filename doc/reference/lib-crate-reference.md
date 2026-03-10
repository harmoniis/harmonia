# Lib Crate Reference

This inventory follows the current Cargo workspace members in `../../Cargo.toml`.

## Pillar Layout

1. `lib/core` - always-on infrastructure.
2. `lib/backends` - provider/storage/http adapters.
3. `lib/tools` - capability plugins.
4. `lib/frontends` - communication channels loaded by gateway/baseband.

## Core (`lib/core`)

| Path | Purpose |
|---|---|
| `lib/core/phoenix` | supervisor binary for restart/rollout lifecycle |
| `lib/core/ouroboros` | crash history + patch artifact subsystem |
| `lib/core/vault` | zero-knowledge secret store with encryption at rest and audit logging |
| `lib/core/memory` | memory storage primitives |
| `lib/core/git-ops` | commit/push lineage operations |
| `lib/core/rust-forge` | runtime build/forge support |
| `lib/core/cron-scheduler` | scheduling primitives |
| `lib/core/recovery` | recovery ledger/event utilities |
| `lib/core/fs` | sandboxed filesystem operations |
| `lib/core/parallel-agents` | API + tmux swarm execution engine |
| `lib/core/harmonic-matrix` | route constraint + telemetry engine with security-aware routing (`route_allowed_with_context`) |
| `lib/core/config-store` | SQLite-backed scoped configuration store with in-memory cache, component access policies, and env var fallback chain |
| `lib/core/tailnet` | tailscale mesh transport layer with HMAC-SHA256 authentication and replay protection |
| `lib/core/baseband-channel-protocol` | shared Baseband Channel Protocol envelope types for gateway/frontend boundaries |
| `lib/core/gateway` | signal baseband + frontend registry with capabilities parsing, metadata enrichment, A2UI-aware signal emission, and inline dissonance scoring |
| `lib/core/signal-integrity` | shared injection detection, dissonance scoring, and boundary wrapping for external data |
| `lib/core/admin-intent` | Ed25519 signature verification for privileged admin mutations |
| `lib/core/chronicle` | Graph-native knowledge base with time-series observability, concept graph SQL traversal, and pressure-aware GC |

## Backends (`lib/backends`)

| Path | Purpose |
|---|---|
| `lib/backends/llms/provider-protocol` | Shared model pool protocol, metrics DB, FFI helpers for all LLM backends |
| `lib/backends/llms/provider-router` | Generic provider router surface consumed by Lisp; currently serves OpenRouter-backed dispatch |
| `lib/backends/llms/openrouter` | Universal LLM gateway via OpenRouter with background model catalogue sync |
| `lib/backends/llms/openai` | OpenAI native backend |
| `lib/backends/llms/anthropic` | Anthropic Messages API backend |
| `lib/backends/llms/xai` | xAI / Grok backend with reasoning and web-search support |
| `lib/backends/llms/google-ai-studio` | Google AI Studio (Gemini) backend |
| `lib/backends/llms/google-vertex` | Google Vertex AI backend (Bearer token auth) |
| `lib/backends/llms/amazon-bedrock` | Amazon Bedrock / Nova backend (AWS CLI) |
| `lib/backends/llms/groq` | Groq backend |
| `lib/backends/llms/alibaba` | Alibaba / DashScope (Qwen) backend |
| `lib/backends/storage/s3` | storage adapter for artifact/object persistence |
| `lib/backends/http` | shared HTTP adapter crate |

## Tools (`lib/tools`)

| Path | Purpose |
|---|---|
| `lib/tools/browser` | secure browser and extraction macros |
| `lib/tools/search-exa` | Exa search integration with boundary-wrapped results |
| `lib/tools/search-brave` | Brave search fallback integration with boundary-wrapped results |
| `lib/tools/whisper` | speech-to-text integration |
| `lib/tools/elevenlabs` | text-to-speech integration |
| `lib/tools/social` | social connector scaffolding |

## Frontends (`lib/frontends`)

| Path | Purpose |
|---|---|
| `lib/frontends/push` | HTTP webhook push notification utility (rlib, consumed by mqtt-client — not a cdylib frontend) |
| `lib/frontends/mqtt-client` | MQTT channel frontend with device registry, persisted offline queue, remote trusted-device cache, push integration, and agent/client fingerprint validation |
| `lib/frontends/whatsapp` | WhatsApp channel frontend |
| `lib/frontends/telegram` | Telegram channel frontend |
| `lib/frontends/slack` | Slack channel frontend |
| `lib/frontends/discord` | Discord channel frontend |
| `lib/frontends/signal` | Signal channel frontend via signal-cli bridge |
| `lib/frontends/mattermost` | Mattermost channel frontend |
| `lib/frontends/nostr` | Nostr channel frontend |
| `lib/frontends/email-client` | email channel frontend |
| `lib/frontends/tui` | local terminal frontend |
| `lib/frontends/imessage` | iMessage channel frontend |
| `lib/frontends/tailscale` | tailnet-backed inter-node channel frontend |

## Cross-Reference To Canonical Docs

1. Detailed architecture and FFI narrative: `../../../doc/agent/genesis/ARCHITECTURE.md`
2. Gateway/frontend contract: `../../../doc/agent/genesis/GATEWAY.md`
3. Swarm and tmux APIs: `../../../doc/agent/genesis/SWARM.md`
4. Current tool/runtime state: `../../../doc/agent/evolution/latest/TOOLS.md`
