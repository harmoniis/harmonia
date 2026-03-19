# Runtime Architecture

## Runtime Topology

Harmonia runtime is Lisp-first orchestration with Rust execution ports.

- Lisp coordinates prompts, memory, model selection, routing, and loop control.
- Rust crates provide external capabilities via `harmonia-runtime`, a single Rust binary containing all ractor actors.
- SBCL communicates with `harmonia-runtime` over IPC (Unix domain socket at `$STATE_ROOT/runtime.sock`, length-prefixed s-expressions).

### Process Topology

```
Phoenix (harmonia-phoenix, ractor supervisor, health at 127.0.0.1:9100)
  ├─ harmonia-runtime (single Rust binary, all ractor actors)
  │     ├─ RuntimeSupervisor      (actor registry, IPC dispatch, supervisor restart)
  │     ├─ SbclBridgeActor        (drain queue for SBCL)
  │     ├─ GatewayActor           (poll_baseband, route signals)
  │     ├─ ChronicleActor         (DB init, periodic GC)
  │     ├─ TailnetActor           (mesh listener, poll, route)
  │     ├─ SignalogradActor       (kernel init, observe, status)
  │     ├─ ObservabilityActor     (trace batch management)
  │     ├─ HarmonicMatrixActor    (matrix topology, route constraints, telemetry)
  │     └─ IPC listener (Unix socket, length-prefixed sexp)
  │           └─ dispatch.rs routes to: vault, config, chronicle, gateway,
  │              signalograd, tailnet, harmonic-matrix (689 lines, 50+ ops)
  │
  ├─ sbcl-agent (SBCL/Common Lisp orchestrator)
  │     ├─ ipc-client.lisp        (socket transport, auto-reconnect)
  │     ├─ ipc-ports.lisp         (ipc-vault-*, ipc-config-*, etc.)
  │     └─ 14 port files (all CFFI removed, all use IPC)
  │
  └─ provision-server
```

Data flow: SBCL → ipc-call → Unix socket → dispatch.rs → crate API → reply → SBCL

Tick loop: gateway-poll → process-prompt → memory-heartbeat → harmonic-step → gateway-flush

Harmonic state: signalograd observe/feedback → harmonic-machine state transitions → chronicle record

Phoenix (`lib/core/phoenix/`) is a ractor-based process supervisor with a health HTTP endpoint at `127.0.0.1:9100`. It writes a pidfile and manages all child processes. The RuntimeSupervisor implements automatic restart for all 8 component actors — if any actor crashes, the supervisor respawns it without requiring a full process restart.

CLI lifecycle commands:
- `harmonia start` → Phoenix → spawns runtime + sbcl-agent + provision-server
- `harmonia stop` → SIGTERM to Phoenix → graceful shutdown cascade
- `harmonia status` → queries Phoenix health at `127.0.0.1:9100`
- Self-diagnosis: `/status` (TUI) or `/diagnose` (TUI) — shows Phoenix health + runtime + modules + errors
- Health endpoint: `GET /health` (JSON), `GET /health/ready` (200/503)

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
   - swarm, evolution, chronicle, signalograd.
7. Restore the evolution-matched `signalograd` checkpoint if present, otherwise continue from live working state.

## Deterministic Tick Model

Main control loop: `src/core/loop.lisp`.

Per-tick actions (inline, zero allocation):

1. `%tick-gateway-poll`
2. `%tick-tailnet-poll`
3. `%tick-actor-supervisor`
4. `%tick-process-prompt`
5. `%tick-actor-deliver`
6. `memory-heartbeat`
7. `harmonic-step`
8. `%tick-chronicle-flush`
9. `%tick-gateway-flush`
10. `%tick-tailnet-flush`

Each action is wrapped in `%supervised-action` (Erlang-style). Errors are caught, recorded to the error ring buffer, and the tick continues. The loop never crashes.

Adaptive cooldown: after 10 consecutive error ticks, sleep interval increases 5x to prevent error storms. Outer `handler-case` in `run-loop` catches even tick-level failures.

## Gateway Signal Processing

During `:gateway-poll`, the gateway first intercepts all /commands via unified command dispatch (`command_dispatch.rs`):

- Native commands (`/wallet`, `/identity`, `/help`) are handled entirely in Rust.
- Delegated commands (`/status`, `/backends`, `/frontends`, `/tools`, `/chronicle`, `/metrics`, `/security`, `/feedback`, `/exit`) are routed to a Lisp-registered callback.
- Command responses are sent back to the originating frontend. Command envelopes are filtered out.

Only non-command envelopes pass through to the Lisp orchestrator as **Baseband Channel Protocol** envelopes.

- Each envelope carries typed `:channel`, `:peer`, `:body`, `:capabilities`, `:security`, `:audit`, and `:transport` sections.
- Frontend-driver details stay gateway-private operational data; Lisp reasons over channel semantics, peer identity, capabilities, and security context.

Signals carry a `dissonance` score (0.0-1.0) computed at parse time. High dissonance signals are attenuated in security-aware routing.

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
4. If the typed signal carries A2UI capability, A2UI component catalog is injected.
5. Conductor checks direct tool commands first (`tool op=...`).
6. **Policy gate**: Before executing any privileged tool op, `%policy-gate` checks the originating signal's taint and security label. Tainted origins are denied for privileged operations.
7. If no direct tool command, model is selected by policy and backend completion runs through the provider-router port.
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

`signalograd` is coupled to this phase:

1. chronicle records the finished cycle
2. Lisp sends `signalograd` feedback for the previous applied projection
3. Lisp sends a new telemetry observation
4. Rust advances the chaotic reservoir / attractor memory state
5. Rust posts a bounded proposal through the unified actor mailbox
6. Lisp applies that proposal only on the next cycle after policy clamps

This makes the adaptive layer causal, auditable, and actor-model aligned.

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
- `introspect-libs` — all loaded library modules with crash counts.
- `%cargo-build-component` — self-compilation of individual crates.

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
