# System Map

## Runtime Topology

Harmonia is layered as a constrained orchestration system:

1. `Governance Layer`
- Genesis constraints and identity: `../../../doc/agent/genesis/*`
- Evolution state and scoring: `../../../doc/agent/evolution/latest/*`

2. `Lisp Runtime Layer` (`src/`)
- `core/`: boot, loop, policy, harmony machine, evolution versioning
  - `introspection`: runtime self-knowledge, self-compilation, hot-reload, error ring, library tracking
- `dna/`: constitutional prompt and identity guardrails
- `orchestrator/`: prompt assembly + conductor planning/coordination (swarm-first execution)
- `memory/`: state, concept map, compression
- `ports/`: capability boundaries to Rust C-ABI
- Supervision: Erlang-style `%supervised-action` wrapping every tick action, error ring, adaptive cooldown, gateway FFI catch_unwind

3. `Rust Capability Layer` (`lib/`)
- `core/`: vault, gateway, matrix, recovery, forge, etc.
- `backends/`: llm/storage/http adapters
- `tools/`: search, browser, voice tools
- `frontends/`: channels loaded via gateway/baseband

4. `Signal/Channel Layer`
- Gateway/baseband polls frontends and emits capability-enriched, metadata-annotated signals.
- Signals carry per-frontend capabilities (from baseband config) and per-message metadata (from frontend poll output).
- Frontends include local (`tui`), network (`mqtt`), and mesh (`tailscale`).
- A2UI dispatch is capabilities-driven — any frontend declaring `:a2ui` capability gets rich UI treatment.

5. `Security Layer`
- Security kernel: typed signal dispatch, deterministic policy gate, taint propagation via `*current-originating-signal*`.
- Adaptive shell: dissonance scoring at gateway, security-aware harmonic routing, `:security-audit` phase in harmonic machine.
- Transport security: tailnet HMAC authentication, MQTT fingerprint validation, vault encryption at rest.
- Policy config: `:security` section in `config/harmony-policy.sexp`.

6. `Experience Layer`
- Runtime logs, memory entries, matrix telemetry, recovery logs, and evolution snapshots feed future behavior.
- Chronicle knowledge base (`lib/core/chronicle`) durably records harmonic snapshots, memory evolution, delegation decisions, and concept graph decompositions in SQLite — queryable with complex SQL returning s-expressions. Pressure-aware GC preserves high-signal data (inflection points, failures, recoveries) while thinning noise.

## Core Runtime Flows

## Prompt-Orchestration Flow

1. Prompt enters queue (`src/core/loop.lisp`).
2. Conductor assembles DNA + memory context (`src/orchestrator/prompt-assembly.lisp`).
3. Dispatch by type:
- `harmonia-signal` → `orchestrate-signal` (boundary-wrap, LLM interpretation, policy-gated tool proposals), or
- `string` → `orchestrate-prompt` (internal, direct tool dispatch if explicit tool op is present).
4. For non-tool execution, conductor delegates to swarm (`parallel-solve`) using model escalation chain and optional context summarization handoff.
5. Policy gate checks taint and security label before privileged tool execution.
6. Result is scored and persisted to memory and chronicle delegation log.
7. Matrix route/event telemetry is updated.

## Gateway/Baseband Signal Flow

1. Baseband polls registered frontends (`src/ports/baseband.lisp`).
2. Gateway parses 3-field poll output (`sub_channel\tpayload[\tmetadata]`) and enriches signals with frontend capabilities.
3. Loop.lisp extracts nested `:channel` (`:frontend` + `:sub-channel`), `:security`, `:capabilities`, `:metadata`.
4. Conductor checks signal capabilities for A2UI and injects component catalog (`config/a2ui-catalog.sexp`) when present.
5. Outbound messages are flushed by gateway-send with A2UI text fallback for non-capable frontends.
6. Frontend auto-load policy comes from `config/baseband.sexp`.

## Evolution Flow

1. Harmonic machine computes readiness context (`src/core/harmonic-machine.lisp`).
2. Rewrite trigger bookkeeping occurs (`src/core/rewrite.lisp`).
3. Evolution mode dispatch via `src/ports/evolution.lisp`:
- `:source-rewrite` (Ouroboros patch path), or
- `:artifact-rollout` (Phoenix-supervised rollout path).
4. Version snapshots are managed in `src/core/evolution-versioning.lisp` and `src/boot/evolution/`.

## Source-of-Truth Concept Map

| Concept Family | Primary Source Docs |
|---|---|
| Harmonic philosophy, laws, attractors | `../../../doc/agent/genesis/HARMONIC_THEORY.md` |
| Architecture and FFI contract | `../../../doc/agent/genesis/ARCHITECTURE.md` |
| Gateway/baseband and signal semantics | `../../../doc/agent/genesis/GATEWAY.md` |
| Swarm tiers and tmux orchestration | `../../../doc/agent/genesis/SWARM.md` |
| Self rewrite protocol | `../../../doc/agent/genesis/SELF_REWRITE.md` |
| UI/UX and A2UI intent | `../../../doc/agent/genesis/UIUX.md`, `../../../doc/agent/genesis/A2UI_SPEC.md` |
| Runtime matrix policy | `../../../doc/agent/evolution/latest/HARMONIC_MATRIX.md` |
| Swarm/model policy | `../../../doc/agent/evolution/latest/SWARM_POLICY.md` |
| Recovery role split | `../../../doc/agent/evolution/latest/RECOVERY.md` |
| Evolution/versioning process | `../../../doc/agent/evolution/EVOLUTION.md` |
| Chronicle knowledge base | `lib/core/chronicle/` — graph-native observability, SQL-traversable concept graphs |

## Architectural Guardrails

1. Lisp remains orchestration-first; external I/O stays in Rust crates.
2. Secrets are vault-bound and must not leak into prompt/memory logs.
3. Route permissions are matrix-constrained, not ad-hoc.
4. Evolution requires explicit safety gates and rollback path.
5. Security kernel gates are deterministic and non-bypassable for privileged operations.
6. External signal taint must propagate through the entire orchestration chain.
7. Reference docs must preserve, not truncate, concept coverage from canonical docs.
