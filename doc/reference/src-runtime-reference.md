# Src Runtime Reference

## Lisp Module Inventory

## Core (`src/core`)

| File | Role |
|---|---|
| `src/core/boot.lisp` | package exports, load order, startup sequence, port initialization |
| `src/core/loop.lisp` | deterministic tick action/effect loop, gateway-poll produces `harmonia-signal` structs with taint labels |
| `src/core/state.lisp` | runtime state struct, lifecycle fields, `harmonia-signal` struct, `*current-originating-signal*` |
| `src/core/tools.lisp` | declarative tool registry from `config/tools.sexp` |
| `src/core/model-policy.lisp` | task classification + seed/model scoring + provider-scoped routing + CLI cooloff/quota tracking |
| `src/core/harmony-policy.lisp` | mutable harmonic policy load/get/set/save |
| `src/core/harmonic-machine.lisp` | multi-phase harmonic planner, rewrite readiness, `:security-audit` phase |
| `src/core/signalograd.lisp` | telemetry-first reflection layer, proposal clamps, checkpoint/restore orchestration, chronicle audit bridge |
| `src/core/rewrite.lisp` | vitruvian-gated rewrite hooks; thresholds from DNA constraints; logs to Ouroboros |
| `src/core/recovery-cascade.lisp` | health tracking, heartbeat (every 10 cycles), dreaming (every 30 cycles), evolutionary circuit breakers |
| `src/core/sexp-eval.lisp` | restricted Lisp evaluator, REPL loop, 20+ primitives (recall, git-*, ouroboros-*, dream, evolve) |
| `src/core/evolution-versioning.lisp` | evolution snapshot version state and snapshot mechanics |
| `src/core/system-commands.lisp` | Lisp-side command handlers for gateway-delegated /commands, %gateway-dispatch-command callback entry point |
| `src/core/conditions.lisp` | condition/error taxonomy helpers |
| `src/core/introspection.lisp` | runtime self-knowledge: platform detection, path introspection, library tracking, error ring, self-compilation, diagnostic snapshots |
| `src/core/supervision-state.lisp` | shared supervision counters loaded before readers (`*tick-error-count*`, cooldown state) |

## Orchestration (`src/orchestrator`)

| File | Role |
|---|---|
| `src/orchestrator/prompt-assembly.lisp` | DNA/memory/system context assembly |
| `src/orchestrator/conductor.lisp` | split dispatch (signal vs prompt), policy gate, safe parsers, tool ops, swarm-first delegation, context summarization handoff, matrix observation, memory/chronicle persistence |

## Memory/Harmony/DNA

| File | Role |
|---|---|
| `src/memory/store.lisp` | memory API surface and store integration |
| `src/harmony/scorer.lisp` | harmonic score function |
| `src/dna/dna.lisp` | genome: genes (function refs), constraints (hard limits), bounds (epigenetic ranges), foundation (concept names). DNA is code, not text. |

## Ports (`src/ports`)

| Port File | Boundary | Current Rust Side |
|---|---|---|
| `src/ports/vault.lisp` | secret symbol store and lookup | `lib/core/vault` |
| `src/ports/store.lisp` | runtime non-secret KV config | `lib/core/config-store` |
| `src/ports/router.lisp` | LLM completion router (OpenRouter + native provider adapters) | `lib/backends/llms/openrouter` |
| `src/ports/matrix.lisp` | route constraints + telemetry | `lib/core/harmonic-matrix` |
| `src/ports/ouroboros.lisp` | self-healing crash ledger and patch writing via IPC actor | `lib/core/ouroboros` |
| `src/ports/tool-runtime.lisp` | search/voice tool dispatch | `lib/tools/*` |
| `src/ports/baseband.lisp` | unified command dispatch callback, frontend registration, signal polling/sending | `lib/core/gateway` |
| `src/ports/swarm.lisp` | parallel agents + tmux swarm control | `lib/core/parallel-agents` |
| `src/ports/evolution.lisp` | source-rewrite/artifact-rollout mode dispatch | `lib/core/ouroboros` + phoenix supervision model |
| `src/ports/chronicle.lisp` | graph-native knowledge base queries, harmonic/memory/delegation recording, concept graph SQL traversal | `lib/core/chronicle` |
| `src/ports/signalograd.lisp` | chaotic advisory kernel IPC (`observe`, `feedback`, `checkpoint`, `restore`, `status`) | `lib/core/signalograd` |
| `src/ports/mempalace.lisp` | graph-structured knowledge palace with AAAK compression | `lib/core/mempalace` |
| `src/ports/terraphon.lisp` | platform datamining tools with cross-node extraction | `lib/core/terraphon` |
| `src/ports/observability.lisp` | provider-agnostic distributed tracing; fire-and-forget `ipc-cast`, client-side UUID run-ids, `with-trace` macro | `lib/core/observability` |

## Rust Runtime (`lib/core/runtime/src`)

| File | Role |
|---|---|
| `supervisor.rs` | RuntimeSupervisor actor — registry, IPC component dispatch, child actor lifecycle |
| `dispatch.rs` | IPC message dispatch — routes to vault, config, chronicle, gateway, signalograd, tailnet, harmonic-matrix, observability, provider-router, parallel, ouroboros, mempalace, terraphon |
| `bridge.rs` | SbclBridgeActor — Unix socket connection handler, drain queue for SBCL |
| `ipc.rs` | IPC listener — Unix socket accept loop, length-prefixed sexp framing |
| `actors.rs` | Actor definitions — GatewayActor, ChronicleActor, TailnetActor, SignalogradActor, ObservabilityActor (ObsMsg, sampling, correlation), HarmonicMatrixActor, VaultActor, ConfigActor, ProviderRouterActor, ParallelActor, RouterActor, OuroborosActor, MempalaceActor, TerraphonActor |
| `msg.rs` | Actor message types and routing enums |

All crates are compiled as rlib and linked into the single `harmonia-runtime` binary. No cdylib shared libraries.

## Boot Knowledge (`src/boot`)

| Path | Role |
|---|---|
| `src/boot/genesis/*` | concise runtime-adjacent genesis corpus |
| `src/boot/evolution/latest/*` | mutable current evolution snapshot, including `signalograd.sexp` checkpoint artifacts |
| `src/boot/evolution/versions/vN/*` | immutable version history, including version-matched `signalograd.sexp` |
| `src/boot/evolution/version.sexp` | current version integer loaded at boot |

## Boot Sequence (Current)

Based on `src/core/boot.lisp`:

1. Load state/tools/DNA/memory/harmony modules.
2. Load `supervision-state.lisp`, `signalograd.lisp`, and `evolution-versioning.lisp`.
3. Load ports in order: vault -> store -> harmony-policy -> model-policy -> router -> ouroboros -> matrix -> tool-runtime -> baseband -> swarm -> evolution -> chronicle -> signalograd -> memory-field -> mempalace -> terraphon.
4. Initialize runtime and DNA guard.
5. Load evolution version state (`init-evolution-versioning`).
6. Initialize ports, bootstrap matrix, and register configured frontends from `config/baseband.sexp`.
7. Initialize chronicle and signalograd, then restore the evolution-matched `signalograd` checkpoint if available.
8. Enter run loop if requested.

## Runtime Tick Actions

From `src/core/loop.lisp`, action order is:

1. gateway poll (includes unified command interception — commands handled/delegated, only agent prompts pass through)
2. tailnet poll
3. actor supervisor mailbox drain
4. queued prompt processing
5. actor result delivery
6. memory heartbeat
7. harmonic state step
8. chronicle flush
9. gateway flush
10. tailnet flush

`signalograd` lives inside the harmonic cycle rather than as a standalone tick action:

- on `:stabilize`, chronicle records the finished cycle first
- Lisp sends `signalograd` feedback for the previous projection
- Lisp sends a new telemetry observation
- Rust posts a proposal into the unified actor mailbox
- the next actor-supervisor pass applies that bounded overlay for the following cycle

This deterministic order is critical for reproducibility and telemetry interpretation.

## Primary Canonical Cross-References

1. Runtime architecture: `../genesis/runtime-architecture.md`
2. Gateway/baseband semantics: `../genesis/gateway-frontends.md`
3. Ports and IPC: `../genesis/ports-and-ffi.md`
4. Evolution state: `../evolution/current-state.md`
5. Rewrite roadmap: `../evolution/rewrite-roadmap.md`
