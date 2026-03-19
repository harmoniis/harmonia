# Ports And IPC

Harmonia uses a port-style architecture in Lisp (`src/ports/*.lisp`).
Each port encapsulates one capability contract and communicates with Rust via IPC over a Unix domain socket (`$STATE_ROOT/runtime.sock`).

> **Historical note**: Prior to the IPC architecture, ports communicated with Rust crates via C-ABI FFI (`#[no_mangle] extern "C"` exports) loaded through CFFI/dlopen as `.so`/`.dylib` shared libraries. That FFI layer has been fully removed. All inter-language communication now uses length-prefixed s-expression messages over the Unix domain socket provided by `harmonia-runtime`.

## Port Map

| Port | Lisp File | Primary Rust Crate(s) | Responsibility |
|---|---|---|---|
| Vault | `src/ports/vault.lisp` | `lib/core/vault` | Secret storage and lookup |
| Store | `src/ports/store.lisp` | `lib/core/config-store` | Mutable non-secret runtime config |
| Router | `src/ports/router.lisp` | `lib/backends/llms/provider-router` | Generic LLM provider router over provider adapters |
| Lineage | `src/ports/lineage.lisp` | `lib/core/git-ops` | Commit/push operations |
| Matrix | `src/ports/matrix.lisp` | `lib/core/harmonic-matrix` | Route constraints + telemetry |
| Tool Channel | `src/ports/tool-channel.lisp` | `lib/core/gateway` (ToolRegistry) + `lib/core/tool-channel-protocol` + tool crates | Protocolised tool invocation via ToolVtable contract |
| Voice Runtime | `src/ports/voice-runtime.lisp` | `lib/backends/voice/voice-router` | Speech-to-text and text-to-speech via voice backend routing |
| Baseband | `src/ports/baseband.lisp` | `lib/core/gateway` + `lib/core/baseband-channel-protocol` + frontend crates | Unified command dispatch, typed Baseband Channel Protocol envelopes, channel send/status, gateway admin lifecycle |
| Swarm | `src/ports/swarm.lisp` | `lib/core/parallel-agents` | Parallel and tmux subagents |
| Evolution | `src/ports/evolution.lisp` | `lib/core/ouroboros` (+ phoenix process) | Rewrite prep/execute/rollback |
| Chronicle | `src/ports/chronicle.lisp` | `lib/core/chronicle` | Graph-native knowledge base, time-series observability, concept graph SQL traversal |
| Signalograd | `src/ports/signalograd.lisp` | `lib/core/signalograd` | chaos-computing advisory kernel: observe, feedback, checkpoint, restore, status |
| Signal Integrity | (used by gateway + conductor) | `lib/core/signal-integrity` | Shared injection detection + dissonance scoring |
| Admin Intent | (used by conductor policy gate) | `lib/core/admin-intent` | Ed25519 admin intent signature verification |

## IPC Transport

SBCL communicates with `harmonia-runtime` via a Unix domain socket at `$STATE_ROOT/runtime.sock`:

- Messages are length-prefixed s-expressions.
- Socket permissions are restricted to owner-only (0600).
- The `SbclBridgeActor` inside `harmonia-runtime` handles the Rust side of the socket, with drain-queue semantics.
- `dispatch.rs` (689 lines, 50+ ops) routes IPC messages to 7 component domains: **vault**, **config**, **chronicle**, **gateway**, **signalograd**, **tailnet**, **harmonic-matrix**.
- SBCL side: `ipc-client.lisp` (socket transport, auto-reconnect), `ipc-ports.lisp` (typed port accessors for `ipc-vault-*`, `ipc-config-*`, etc.), and all 14 port files use IPC exclusively.

## Core Contract Rule

All external effects go through one of these ports.

`signalograd` is a special case inside that rule: it is not an external network effect port, but it is still kept behind a port boundary so the adaptive kernel remains explicit, inspectable, and replaceable.

That guarantees:

- traceability in Lisp,
- bounded IPC surfaces,
- and policy enforcement (matrix + vault + config) at orchestration points.
