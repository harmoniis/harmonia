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
| `lib/core/transport-auth` | shared transport trust helpers for TLS material loading, certificate fingerprint normalization, and config-store backed trusted-identity validation used by MQTT and HTTP/2 |
| `lib/core/tailnet` | tailscale mesh transport layer with HMAC-SHA256 authentication and replay protection |
| `lib/core/baseband-channel-protocol` | shared Baseband Channel Protocol envelope types for gateway/frontend boundaries |
| `lib/core/gateway` | unified command dispatch + signal baseband + frontend registry with capabilities parsing, metadata enrichment, A2UI-aware signal emission, inline dissonance scoring, and default deny-all sender policy for messaging frontends |
| `lib/core/signal-integrity` | shared injection detection, dissonance scoring, and boundary wrapping for external data |
| `lib/core/admin-intent` | Ed25519 signature verification for privileged admin mutations |
| `lib/core/chronicle` | Graph-native knowledge base with time-series observability, concept graph SQL traversal, and pressure-aware GC |
| `lib/core/tool-channel-protocol` | standardised request/result types for tool channel communication |
| `lib/core/signalograd` | tiny chaos-computing advisory kernel with Lorenz-style reservoir dynamics, Hopfield-like attractor memory, local online learning, and evolution checkpoint persistence |
| `lib/core/qr-terminal` | QR code terminal rendering utility for device linking |
| `lib/core/node-rpc` | node-to-node RPC for remote frontend pairing via Tailscale mesh |

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
| `lib/backends/voice/voice-protocol` | shared voice provider protocol, timeout helpers, multipart upload |
| `lib/backends/voice/voice-router` | voice backend routing — STT + TTS provider dispatch with fallback chains |
| `lib/backends/voice/whisper` | speech-to-text backend (Groq primary, OpenAI fallback) |
| `lib/backends/voice/elevenlabs` | text-to-speech backend via ElevenLabs API |

## Tools (`lib/tools`)

| Path | Purpose |
|---|---|
| `lib/tools/browser` | headless browser with Chrome CDP, stealth anti-detection engine, and session pooling |
| `lib/tools/search-exa` | Exa search integration with boundary-wrapped results |
| `lib/tools/search-brave` | Brave search fallback integration with boundary-wrapped results |
| `lib/tools/hfetch` | secure HTTP client with SSRF protection, injection detection, and dissonance scoring (library + CLI) |
| `lib/tools/zoom` | Zoom meeting automation via browser (join, leave, transcript, chat, participants) |

## Frontends (`lib/frontends`)

| Path | Purpose |
|---|---|
| `lib/frontends/push` | HTTP webhook push notification utility consumed by mqtt-client |
| `lib/frontends/mqtt-client` | MQTT channel frontend with device registry, persisted offline queue, remote trusted-device cache, push integration, and agent/client fingerprint validation |
| `lib/frontends/http2-mtls` | HTTP/2-only mutual-TLS streaming frontend with per-stream route keys `<identity>/<session>/<channel>` and authenticated metadata injection into baseband |
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

1. Detailed architecture and FFI narrative: `../genesis/runtime-architecture.md`
2. Gateway/frontend contract: `../genesis/gateway-frontends.md`
3. Ports and FFI mapping: `../genesis/ports-and-ffi.md`
