# Src Runtime Reference

## Lisp Module Inventory

## Core (`src/core`)

| File | Role |
|---|---|
| `src/core/boot.lisp` | package exports, load order, startup sequence, port initialization |
| `src/core/loop.lisp` | deterministic tick action/effect loop, gateway-poll produces `harmonia-signal` structs with taint labels |
| `src/core/state.lisp` | runtime state struct, lifecycle fields, `harmonia-signal` struct, `*current-originating-signal*` |
| `src/core/tools.lisp` | declarative tool registry from `config/tools.sexp` |
| `src/core/model-policy.lisp` | task classification + model scoring + routing |
| `src/core/harmony-policy.lisp` | mutable harmonic policy load/get/set/save |
| `src/core/harmonic-machine.lisp` | multi-phase harmonic planner, rewrite readiness, `:security-audit` phase |
| `src/core/rewrite.lisp` | rewrite trigger bookkeeping hooks |
| `src/core/evolution-versioning.lisp` | evolution snapshot version state and snapshot mechanics |
| `src/core/conditions.lisp` | condition/error taxonomy helpers |

## Orchestration (`src/orchestrator`)

| File | Role |
|---|---|
| `src/orchestrator/prompt-assembly.lisp` | DNA/memory/system context assembly |
| `src/orchestrator/conductor.lisp` | split dispatch (signal vs prompt), policy gate, safe parsers, tool ops, model dispatch, matrix observation, memory persistence |

## Memory/Harmony/DNA

| File | Role |
|---|---|
| `src/memory/store.lisp` | memory API surface and store integration |
| `src/harmony/scorer.lisp` | harmonic score function |
| `src/dna/dna.lisp` | constitutional identity prompt and invariants |

## Ports (`src/ports`)

| Port File | Boundary | Current Rust Side |
|---|---|---|
| `src/ports/vault.lisp` | secret symbol store and lookup | `lib/core/vault` |
| `src/ports/store.lisp` | runtime non-secret KV config | `lib/core/config-store` |
| `src/ports/router.lisp` | LLM completion router (OpenRouter + native provider adapters) | `lib/backends/llms/openrouter` |
| `src/ports/lineage.lisp` | commit/push lineage ops | `lib/core/git-ops` |
| `src/ports/matrix.lisp` | route constraints + telemetry | `lib/core/harmonic-matrix` |
| `src/ports/tool-runtime.lisp` | search/voice tool dispatch | `lib/tools/*` |
| `src/ports/baseband.lisp` | frontend registration + signal polling/sending | `lib/core/gateway` |
| `src/ports/swarm.lisp` | parallel agents + tmux swarm control | `lib/core/parallel-agents` |
| `src/ports/evolution.lisp` | source-rewrite/artifact-rollout mode dispatch | `lib/core/ouroboros` + phoenix supervision model |

## Boot Knowledge (`src/boot`)

| Path | Role |
|---|---|
| `src/boot/genesis/*` | concise runtime-adjacent genesis corpus |
| `src/boot/evolution/latest/*` | mutable current evolution snapshot |
| `src/boot/evolution/versions/vN/*` | immutable version history |
| `src/boot/evolution/version.sexp` | current version integer loaded at boot |

## Boot Sequence (Current)

Based on `src/core/boot.lisp`:

1. Load state/tools/DNA/memory/harmony modules.
2. Load `evolution-versioning.lisp`.
3. Load ports in order: vault -> store -> router -> lineage -> matrix -> tool-runtime -> baseband -> swarm -> evolution.
4. Initialize runtime and DNA guard.
5. Load evolution version state (`init-evolution-versioning`).
6. Initialize ports and bootstrap matrix.
7. Register configured frontends from `config/baseband.sexp`.
8. Enter run loop if requested.

## Runtime Tick Actions

From `src/core/loop.lisp`, action order is:

1. baseband poll — gateway polls all registered frontends, parses 3-field output (`sub_channel\tpayload[\tmetadata]`), enriches signals with frontend capabilities from baseband config. Loop creates `harmonia-signal` structs with `:taint :external` and dissonance scores. Signals enter prompt queue as typed structs, not format-strings.
2. process queued prompt — `orchestrate-once` dispatches by type: `harmonia-signal` → `orchestrate-signal` (binds taint, boundary-wraps payload, policy-gates tool commands); `string` → `orchestrate-prompt` (internal/TUI, direct tool dispatch). A2UI dispatch is capabilities-driven.
3. memory heartbeat
4. harmonic state step (includes `:security-audit` phase)
5. baseband flush — gateway-send checks target frontend capabilities, degrades A2UI payloads to text for non-A2UI frontends.

This deterministic order is critical for reproducibility and telemetry interpretation.

## Primary Canonical Cross-References

1. Runtime architecture narrative: `../../../doc/agent/genesis/ARCHITECTURE.md`
2. Gateway/baseband semantics: `../../../doc/agent/genesis/GATEWAY.md`
3. Swarm mechanics: `../../../doc/agent/genesis/SWARM.md`
4. Self rewrite protocol: `../../../doc/agent/genesis/SELF_REWRITE.md`
5. Evolution policy/runtime details: `../../../doc/agent/evolution/latest/*.md`
