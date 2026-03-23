# System Map

## Runtime Topology

Harmonia is layered as a constrained orchestration system:

1. `Governance Layer`
- Genesis constraints and identity: `../../src/boot/genesis/` (canonical), `../genesis/` (markdown mirror)
- Evolution state and scoring: `../../src/boot/evolution/latest/` (canonical), `../evolution/` (markdown mirror)

2. `Lisp Runtime Layer` (`src/`)
- `core/`: boot, loop, policy, harmony machine, evolution versioning
  - `introspection`: runtime self-knowledge, self-compilation, error ring, library tracking
- `dna/`: constitutional prompt and identity guardrails
- `orchestrator/`: prompt assembly + conductor planning/coordination (swarm-first execution)
- `memory/`: state, concept map, compression
- `ports/`: capability boundaries to Rust via IPC (Unix domain socket)
- Supervision: Erlang-style `%supervised-action` wrapping every tick action, error ring, adaptive cooldown

3. `Rust Capability Layer` (`lib/`)
- `core/runtime`: single Rust binary (`harmonia-runtime`) containing ractor actors + IPC listener (`runtime.sock`)
  - RuntimeSupervisor — actor registry, IPC component dispatch, supervisor restart of failed actors
  - SbclBridgeActor — drain queue for SBCL
  - GatewayActor — poll_baseband, route signals
  - ChronicleActor — DB init, periodic GC
  - TailnetActor — mesh listener, poll, route
  - SignalogradActor — kernel init, observe, status
  - ObservabilityActor — provider-agnostic trace sink (sampling, correlation, batch dispatch to configured provider)
  - HarmonicMatrixActor — matrix topology, route constraints, telemetry
  - VaultActor, ConfigActor, ProviderRouterActor, ParallelActor, RouterActor
  - IPC dispatch routes to: vault, config, chronicle, gateway, signalograd, tailnet, harmonic-matrix, observability, provider-router, parallel
- `core/phoenix`: ractor-based process supervisor, health endpoint (`127.0.0.1:9100`), pidfile management
- `core/`: vault, gateway, matrix, recovery, forge, etc.
- `signalograd`: tiny chaotic advisory kernel with local online learning and evolution checkpoints.
- `backends/`: llm/storage/http adapters
- `tools/`: search, browser, voice tools
- `frontends/`: rlib crates compiled into `harmonia-runtime`
- Data flow: SBCL → ipc-call → Unix socket → dispatch.rs → crate API → reply → SBCL

4. `Signal/Channel Layer`
- Gateway/baseband polls frontends and emits capability-enriched, metadata-annotated signals.
- Signals carry per-frontend capabilities (from baseband config) and per-message metadata (from frontend poll output).
- 14 frontend channels: TUI, MQTT, HTTP/2 mTLS, Tailscale, Telegram, WhatsApp, Signal, iMessage, Slack, Discord, Email, Mattermost, Nostr, and future SMS.
- A2UI dispatch is capabilities-driven — any frontend declaring `:a2ui` capability gets rich UI treatment.

5. `Security Layer`
- Security kernel: typed signal dispatch, deterministic policy gate, taint propagation via `*current-originating-signal*`.
- Gateway sender policy: default deny-all for messaging frontends (email, Slack, Discord, Signal, WhatsApp, iMessage, Telegram, Mattermost, Nostr). Allowlist-based sender filtering at the signal boundary before command interception.
- Adaptive shell: dissonance scoring at gateway, security-aware harmonic routing, `:security-audit` phase in harmonic machine.
- Transport security: tailnet HMAC authentication, MQTT trusted identity validation, HTTP/2 mutual TLS, vault encryption at rest.
- Policy config: `:security` section in `config/harmony-policy.sexp`, sender policy in config-store (`sender-policy` scope).

6. `Experience Layer`
- Runtime logs, memory entries, matrix telemetry, recovery logs, and evolution snapshots feed future behavior.
- Chronicle knowledge base (`lib/core/chronicle`) durably records harmonic snapshots, memory evolution, delegation decisions, and concept graph decompositions in SQLite — queryable with complex SQL returning s-expressions. Pressure-aware GC preserves high-signal data (inflection points, failures, recoveries) while thinning noise.
- `signalograd` adds a bounded epigenetic layer: it learns from telemetry, stores a compact attractor state, emits advisory overlays, and now records `observe` / `feedback` / `proposal` / `checkpoint` / `restore` events into chronicle.

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
2. Gateway applies sender policy filter (`sender_policy.rs`): messaging frontend signals from unlisted senders are dropped before further processing.
3. Gateway intercepts all /commands via unified command dispatch (`command_dispatch.rs`): native Rust handlers or Lisp callback delegation. Command responses sent back to originating frontend.
4. Non-command envelopes pass through. Gateway parses 3-field poll output (`sub_channel\tpayload\tmetadata_sexp`) and enriches signals with frontend capabilities.
5. Sessionful transports keep route identity in `:sub-channel`; HTTP/2 uses `<identity>/<session>/<channel>` so multiple remote sessions can advance concurrently.
6. Loop.lisp extracts nested `:channel` (`:frontend` + `:sub-channel`), `:security`, `:capabilities`, `:metadata`.
7. Conductor checks signal capabilities for A2UI and injects component catalog (`config/a2ui-catalog.sexp`) when present.
8. Outbound messages are flushed by gateway-send with A2UI text fallback for non-capable frontends.
9. Frontend auto-load policy comes from `config/baseband.sexp`.

## Evolution Flow

1. Harmonic machine computes readiness context (`src/core/harmonic-machine.lisp`).
2. Rewrite trigger bookkeeping occurs (`src/core/rewrite.lisp`).
3. `signalograd` observes the stabilized cycle, receives one-step-late feedback, and posts bounded proposals for the next cycle.
4. Evolution mode dispatch via `src/ports/evolution.lisp`:
- `:source-rewrite` (Ouroboros patch path), or
- `:artifact-rollout` (Phoenix-supervised rollout path).
5. Version snapshots are managed in `src/core/evolution-versioning.lisp` and `src/boot/evolution/`, including `signalograd.sexp` checkpoints alongside accepted evolution versions.

## Source-of-Truth Concept Map

| Concept Family | Primary Source Docs |
|---|---|
| Harmonic philosophy, laws, attractors | `../genesis/concepts.md` |
| Architecture and IPC contract | `../genesis/runtime-architecture.md` |
| Gateway/baseband and signal semantics | `../genesis/gateway-frontends.md` |
| Ports and IPC mapping | `../genesis/ports-and-ffi.md` |
| A2UI component catalog | `../../config/a2ui-catalog.sexp` |
| Runtime matrix topology | `../../config/matrix-topology.sexp` |
| Swarm/model policy | `../../config/swarm.sexp`, `../../config/model-policy.sexp` |
| Evolution changelog and state | `../evolution/changelog.md`, `../evolution/current-state.md` |
| Rewrite roadmap | `../evolution/rewrite-roadmap.md` |
| Chronicle knowledge base | `lib/core/chronicle/` — graph-native observability, SQL-traversable concept graphs |

## Architectural Guardrails

1. Lisp remains orchestration-first; external I/O stays in Rust crates.
2. Secrets are vault-bound and must not leak into prompt/memory logs.
3. Route permissions are matrix-constrained, not ad-hoc.
4. Evolution requires explicit safety gates and rollback path.
5. Security kernel gates are deterministic and non-bypassable for privileged operations.
6. External signal taint must propagate through the entire orchestration chain.
7. `signalograd` is advisory only; it cannot become a second sovereign controller.
8. Reference docs must preserve, not truncate, concept coverage from canonical docs.
