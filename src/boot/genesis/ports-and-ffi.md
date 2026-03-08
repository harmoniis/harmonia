# Ports And FFI

Harmonia uses a port-style architecture in Lisp (`src/ports/*.lisp`).
Each port encapsulates one capability contract and binds to one or more Rust crates.

## Port Map

| Port | Lisp File | Primary Rust Crate(s) | Responsibility |
|---|---|---|---|
| Vault | `src/ports/vault.lisp` | `lib/core/vault` | Secret storage and lookup |
| Store | `src/ports/store.lisp` | `lib/core/config-store` | Mutable non-secret runtime config |
| Router | `src/ports/router.lisp` | `lib/backends/llms/openrouter` | LLM completion router (OpenRouter + native provider adapters) |
| Lineage | `src/ports/lineage.lisp` | `lib/core/git-ops` | Commit/push operations |
| Matrix | `src/ports/matrix.lisp` | `lib/core/harmonic-matrix` | Route constraints + telemetry |
| Tool Runtime | `src/ports/tool-runtime.lisp` | `lib/tools/search-*`, `lib/tools/whisper`, `lib/tools/elevenlabs` | Search + voice tools |
| Baseband | `src/ports/baseband.lisp` | `lib/core/gateway` + frontend cdylibs | Frontend registration with capabilities, signal poll (metadata + capabilities enrichment), send with A2UI fallback |
| Swarm | `src/ports/swarm.lisp` | `lib/core/parallel-agents` | Parallel and tmux subagents |
| Evolution | `src/ports/evolution.lisp` | `lib/core/ouroboros` (+ phoenix process) | Rewrite prep/execute/rollback |
| Signal Integrity | (used by gateway + conductor) | `lib/core/signal-integrity` | Shared injection detection + dissonance scoring |
| Admin Intent | (used by conductor policy gate) | `lib/core/admin-intent` | Ed25519 admin intent signature verification |

## Shared Port Infrastructure

Defined in `src/ports/vault.lisp` and reused by all ports:

- `ensure-cffi`: one-time CFFI bootstrap.
- `%release-lib-path`: resolve release dylib paths.
- `%split-lines`: decode newline-returned ffi outputs.

## Core Contract Rule

All external effects go through one of these ports.

That guarantees:

- traceability in Lisp,
- bounded FFI surfaces,
- and policy enforcement (matrix + vault + config) at orchestration points.
