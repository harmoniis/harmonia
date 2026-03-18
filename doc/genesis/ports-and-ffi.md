# Ports And FFI

Harmonia uses a port-style architecture in Lisp (`src/ports/*.lisp`).
Each port encapsulates one capability contract and binds to one or more Rust crates.

## Port Map

| Port | Lisp File | Primary Rust Crate(s) | Responsibility |
|---|---|---|---|
| Vault | `src/ports/vault.lisp` | `lib/core/vault` | Secret storage and lookup |
| Store | `src/ports/store.lisp` | `lib/core/config-store` | Mutable non-secret runtime config |
| Router | `src/ports/router.lisp` | `lib/backends/llms/provider-router` | Generic LLM provider router over provider adapters |
| Lineage | `src/ports/lineage.lisp` | `lib/core/git-ops` | Commit/push operations |
| Matrix | `src/ports/matrix.lisp` | `lib/core/harmonic-matrix` | Route constraints + telemetry |
| Tool Channel | `src/ports/tool-channel.lisp` | `lib/core/gateway` (ToolRegistry) + `lib/core/tool-channel-protocol` + tool cdylibs | Protocolised tool invocation via ToolVtable contract |
| Voice Runtime | `src/ports/voice-runtime.lisp` | `lib/backends/voice/voice-router` | Speech-to-text and text-to-speech via voice backend routing |
| Baseband | `src/ports/baseband.lisp` | `lib/core/gateway` + `lib/core/baseband-channel-protocol` + frontend cdylibs | Unified command dispatch, typed Baseband Channel Protocol envelopes, channel send/status, gateway admin lifecycle |
| Swarm | `src/ports/swarm.lisp` | `lib/core/parallel-agents` | Parallel and tmux subagents |
| Evolution | `src/ports/evolution.lisp` | `lib/core/ouroboros` (+ phoenix process) | Rewrite prep/execute/rollback |
| Chronicle | `src/ports/chronicle.lisp` | `lib/core/chronicle` | Graph-native knowledge base, time-series observability, concept graph SQL traversal |
| Signalograd | `src/ports/signalograd.lisp` | `lib/core/signalograd` | chaos-computing advisory kernel: observe, feedback, checkpoint, restore, status |
| Signal Integrity | (used by gateway + conductor) | `lib/core/signal-integrity` | Shared injection detection + dissonance scoring |
| Admin Intent | (used by conductor policy gate) | `lib/core/admin-intent` | Ed25519 admin intent signature verification |

## Shared Port Infrastructure

Defined in `src/ports/vault.lisp` and reused by all ports:

- `ensure-cffi`: one-time CFFI bootstrap.
- `%release-lib-path`: resolve release dylib paths.
- `%release-lib-roots`: resolve candidate library roots via fallback chain: `HARMONIA_LIB_DIR` env var → `target/release/` → `~/.local/lib/harmonia/`.
- `%split-lines`: decode newline-returned ffi outputs.

## Core Contract Rule

All external effects go through one of these ports.

`signalograd` is a special case inside that rule: it is not an external network effect port, but it is still kept behind a port boundary so the adaptive kernel remains explicit, inspectable, and replaceable.

That guarantees:

- traceability in Lisp,
- bounded FFI surfaces,
- and policy enforcement (matrix + vault + config) at orchestration points.
