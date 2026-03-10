# Runtime Architecture

## Runtime Topology

Harmonia runtime is Lisp-first orchestration with Rust execution ports.

- Lisp coordinates prompts, memory, model selection, routing, and loop control.
- Rust crates provide external capabilities through C-ABI and CFFI.

## Boot Flow

Primary startup entry point: `src/core/boot.lisp`.

Boot sequence:

1. Validate environment safety (`%enforce-genesis-safety`).
2. Initialize runtime state (`make-runtime-state`).
3. Validate DNA (`dna-valid-p`).
4. Register tools from `config/tools.sexp`.
5. Seed soul memory from DNA.
6. Initialize ports in strict order:
   - vault, store, harmony-policy, model-policy,
   - router, lineage, matrix,
   - tool-runtime, baseband frontends,
   - swarm, evolution, chronicle.

## Deterministic Tick Model

Main control loop: `src/core/loop.lisp`.

Per-tick actions (inline, zero allocation):

1. `%tick-gateway-poll` — poll gateway for inbound signals
2. `%tick-process-prompt` — pop and process one prompt (if queue non-empty)
3. `memory-heartbeat` — memory pipeline maintenance
4. `harmonic-step` — harmonic state machine advancement
5. `%tick-gateway-flush` — drain outbound response queue

Each action is wrapped in `%supervised-action` (Erlang-style). Errors are caught, recorded to the error ring buffer, and the tick continues. The loop never crashes.

Adaptive cooldown: after 10 consecutive error ticks, sleep interval increases 5x to prevent error storms. Outer `handler-case` in `run-loop` catches even tick-level failures.

## Gateway Signal Processing

During `:gateway-poll`, the loop reads signal s-expressions and properly navigates the nested structure:

- `:channel` contains `:frontend` and `:sub-channel` (nested plist).
- `:security`, `:capabilities`, `:metadata` are top-level signal fields.

The loop serializes all of these into the `gateway-inbound` prompt string, giving the conductor full signal context including device capabilities and metadata.

Signals also carry a `dissonance` score (0.0-1.0) computed by inline injection scanning at parse time. High dissonance signals are attenuated in security-aware routing.

## Orchestration Flow

Primary orchestrator: `src/orchestrator/conductor.lisp`.

Execution path (split dispatch):

1. Input enters queue (`feed-prompt`) — either `harmonia-signal` struct or string.
2. `orchestrate-once` dispatches by type:
   - `harmonia-signal` → `orchestrate-signal`: binds `*current-originating-signal*`, boundary-wraps payload, sends to LLM. Tool commands in LLM response are **proposed actions** that must pass `%policy-gate`.
   - `string` → `orchestrate-prompt`: internal/TUI prompt. `*current-originating-signal*` is nil (owner trust). May contain direct tool commands.
3. Full LLM prompt is assembled (`dna-compose-llm-prompt`) with:
   - DNA constitution,
   - bootstrap memory block (boundary-wrapped recalls),
   - semantic recall block (boundary-wrapped).
4. If signal has A2UI capability, A2UI component catalog is injected.
5. Conductor checks direct tool commands first (`tool op=...`).
6. **Policy gate**: Before executing any privileged tool op, `%policy-gate` checks the originating signal's taint and security label. Tainted origins are denied for privileged operations.
7. If no direct tool command, model is selected by policy and backend completion runs.
8. For external-origin chains, LLM output is inspected for proposed `tool op=...`; only policy-permitted operations execute (privileged proposals degrade safely).
9. Response is scored (`harmonic-score`) and persisted in memory.
10. Matrix route observations and events are recorded.

## Harmonic State Machine

`src/core/harmonic-machine.lisp` executes a 9-phase cycle:

- observe
- evaluate-global
- evaluate-local
- logistic-balance
- lambdoma-project
- attractor-sync
- rewrite-plan
- **security-audit** (scan injection counts per frontend, update security posture)
- stabilize

Rewrite readiness requires convergence and policy thresholds (`rewrite-plan/*`).

On `:stabilize`, the harmonic machine records a full snapshot (vitruvian scores, chaos dynamics, lorenz attractor, lambdoma convergence, security posture) and a decomposed concept graph into the chronicle knowledge base. This enables SQL-queryable time-travel over the agent's entire harmonic evolution — the agent can recall any past state, traverse concept graph history, and analyze delegation patterns via complex SQL.

## Error Discipline And Self-Repair

Runtime errors are classified and recorded via `src/core/conditions.lisp`:

- compiler,
- backend,
- evolution.

Errors are persisted into memory and matrix event logs instead of silently disappearing.

The supervision layer (`%supervised-action`) catches all `serious-condition` errors, records them to a 64-entry circular error ring (`*error-ring*`), and increments counters. Library crashes are tracked per-library in `*loaded-libs*` with crash counts and status.

Runtime self-knowledge (`src/core/introspection.lisp`) provides:

- Platform and path introspection for autonomous debugging.
- `introspect-runtime` — full diagnostic snapshot.
- `introspect-recent-errors` — last N errors with context.
- `introspect-libs` — all loaded cdylibs with crash counts.
- `%cargo-build-component` — self-compilation of individual crates.
- `%hot-reload-frontend` — rebuild, copy, and re-register a frontend cdylib.

This knowledge is injected into the DNA system prompt via `%runtime-self-knowledge`.

## Security Architecture

Three security layers protect the runtime:

1. **Security Kernel** (deterministic):
   - Typed signal dispatch separates external signals from internal prompts.
   - `%policy-gate` enforces binary allow/deny on 14 privileged operations.
   - `*current-originating-signal*` propagates taint through the reasoning chain.
   - Safe parsers (`%safe-parse-number`, `%safe-parse-policy-value`) eliminate `read-from-string` ACE vectors.
   - Invariant guards enforce non-configurable safety limits.

2. **Adaptive Shell** (harmonic):
   - Dissonance scoring at gateway ingestion (injection pattern detection).
   - Security-aware routing via `route_allowed_with_context`.
   - `:security-audit` phase tracks posture (`:nominal`/`:elevated`/`:alert`).

3. **Transport Security**:
   - Tailnet HMAC-SHA256 authentication with replay protection.
   - MQTT fingerprint validation against vault-stored expected values.
   - Wallet-rooted vault encryption at rest (AES-256-GCM) with audit logging.

Policy configuration: `:security` section in `config/harmony-policy.sexp`.
