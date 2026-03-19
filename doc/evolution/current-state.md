# Current State

Snapshot date: 2026-03-19

## Active Evolution Mode

Current configured mode is `:artifact-rollout` by default, with source rewrite disabled unless explicitly re-enabled by policy/config.

From `src/ports/evolution.lisp`:

- `evolution-prepare` inspects health and crash state.
- `evolution-execute` signals artifact rollout under Phoenix or writes patch artifacts only when source rewrite is enabled.
- `evolution-rollback` records rollback as crash telemetry.

## Runtime Readiness Signals

Rewrite candidate readiness combines:

- harmonic convergence (global/local + lambdoma ratio),
- logistic chaos risk thresholds,
- vitruvian signal/noise gates.

Primary thresholds come from `config/harmony-policy.sexp`:

- `rewrite-plan/signal-min`
- `rewrite-plan/noise-max`
- `rewrite-plan/chaos-max`

## Model/Swarm Policy State

Model selection is task-aware, provider-scoped, and evolves from measured delegation outcomes.

Policy inputs:

- `config/model-policy.sexp`
- `config/swarm.sexp`
- mutable state files under `HARMONIA_STATE_ROOT`.

Current behavior:

- Seed model source is config-store first (no hardcoded runtime seed lock):
  - `model-policy/provider`
  - `model-policy/seed-models`
  - `model-policy/seed-models-<provider>`
- OpenRouter default seed order is:
  1. `inception/mercury-2`
  2. `qwen/qwen3.5-flash-02-23`
  3. `minimax/minimax-m2.5`
  4. `google/gemini-3.1-flash-lite-preview`
- CLI-first routing is active for `:software-dev`, `:coding`, `:critical-reasoning` with timeout/cooloff/quota controls (`:cli-timeout-seconds`, `:cli-cooloff-seconds`, `:cli-quota-patterns`).
- Orchestrator is coordinator-first: non-tool LLM work is delegated through swarm (`parallel-solve`) instead of single direct backend completion.
- Large prompt contexts can be summarized before delegation using `:context-summarizer-model` (`qwen/qwen3.5-flash-02-23`) and `:context-summarizer-threshold-chars`.
- Delegation outcomes (success, latency, cost, harmony/vitruvian signal) are persisted in:
  - `swarm_model_scores.sexp` (model-policy experience state)
  - `chronicle.db` (`delegation_log` table)

Operational entrypoint:

- `harmonia setup --seeds` updates seed provider/model policy only (without re-running full setup).

## Memory Evolution State

Memory pipeline is active with four layers:

- Soul seeding from DNA,
- Daily interaction memory,
- Skill compression and crystallization,
- Temporal journaling (yesterday summary).

Compression and crystal thresholds are policy-controlled (`:memory` section in harmony policy).

## Signalograd State

`signalograd` is now part of the active adaptive runtime.

It is not a conventional deep net and does not use gradient descent. The current target architecture is:

- a Lorenz-style chaotic reservoir / plastic CTRNN regime as temporal compute
- a Hopfield-like attractor memory storing compressed successful states
- tiny bounded readout heads for harmony, routing, memory, evolution, and security shell
- local online learning only: Hebbian, Oja-style normalization, decay, and homeostatic control

Operational rules:

- telemetry-first inputs only in v1
- no raw prompt text as model input
- advisory output only
- deterministic policy, matrix constraints, and privileged security rules remain sovereign

Persistence is two-tier:

- live working model under `${HARMONIA_STATE_ROOT}/signalograd.sexp`
- evolution checkpoints under `src/boot/evolution/latest/signalograd.sexp` and `versions/vN/signalograd.sexp`

On boot, runtime restores the evolution-matched checkpoint first when present, then continues continual local learning into the working-state file.

## Gateway Signal Protocol State

Gateway signals now carry two enrichment layers:

- **Capabilities** (static per-frontend): parsed from `:capabilities` in `config/baseband.sexp` at registration time. Attached to every signal from that frontend. Example: `(:a2ui "1.0" :push "t")`.
- **Metadata** (dynamic per-message): emitted by the frontend as a third tab-field in poll output. Example: `(:platform "ios" :device-id "uuid-123" :a2ui-version "1.0")`.

Poll format is 3-field tab-separated triples: `sub_channel\tpayload\tmetadata_sexp`.

A2UI dispatch is capabilities-driven — the conductor checks signal capabilities, not frontend names. Any frontend declaring `:capabilities (:a2ui "1.0")` gets A2UI context injection and component catalog availability.

A2UI component catalog: `config/a2ui-catalog.sexp` (21 components, lazily loaded and cached by conductor).

Push notifications: `lib/frontends/push` is a utility library consumed by mqtt-client for offline device push via HTTP webhook.

## Unified Command Dispatch (v10)

The gateway is the single interception point for ALL /commands from ALL frontends (TUI, MQTT, Telegram, Tailscale, paired nodes).

Source: `lib/core/gateway/src/command_dispatch.rs`.

Three-tier command handling:

1. **TUI-local** (CLI only, not sent to daemon): `/help`, `/session`, `/resume`, `/clear`, `/log`, `/exit`, `/menu`, `/policies`, `/pair` — handled entirely in `cli/session.rs`.
2. **Native** (Rust gateway): `/wallet`, `/identity`, `/help` — fully executed in Rust with no Lisp involvement.
3. **Delegated** (Lisp callback): `/status`, `/backends`, `/frontends`, `/tools`, `/chronicle`, `/metrics`, `/security`, `/feedback`, `/exit` — routed to `%gateway-dispatch-command` via `CommandQueryFn` callback.

Security enforcement at gateway:
- Read-restricted commands require Owner or Authenticated security label.
- `/exit` is TUI-only.

Exit handling: gateway sets `pending_exit` flag → Lisp checks via `harmonia_gateway_pending_exit` after each poll → calls `(stop runtime)`.

All crates are compiled as rlib and linked into `harmonia-runtime`. No cdylib shared libraries are produced.

## Matrix Enforcement State

All critical orchestrator routes are matrix-gated before invocation.

Matrix topology source of truth:

- seed: `config/matrix-topology.sexp` (includes gateway node)
- mutable state: `${HARMONIA_STATE_ROOT}/matrix-topology.sexp`

## TUI Session System

Sessions are persisted to JSONL event logs with auto-save on every user and assistant message.

### Session Storage

Each session lives at `~/.harmoniis/harmonia/nodes/<label>/sessions/<session-id>/`:
- `session.json` — manifest with id, node_label, created_at_ms, updated_at_ms
- `events.jsonl` — append-only event log (ts_ms, actor, kind, text)

### Resumable Sessions

`/resume` TUI command lists past sessions sorted by most recent, showing timestamps and event counts. On selection, writes `current-session` file and exits — the next `harmonia` invocation connects to the selected session. Also available via `/menu` → Resume Session.

### Auto-Save

Every user message triggers `append_session_event(session, "you", "user", text)`. Every assistant response line triggers `append_session_event(session, "harmonia", "assistant", line)`. The session manifest `updated_at_ms` is updated on each event.

## Gateway Sender Policy

Default deny-all sender filtering is active for messaging frontends at the gateway layer.

### Filtered Frontends

email, slack, discord, mattermost, signal, whatsapp, imessage, telegram, nostr.

### Exempt Frontends

TUI (local), MQTT (device-paired), Tailscale (mesh-authenticated).

### Policy Evaluation Order

1. Non-messaging frontend → allow
2. Self-originated signal (`origin.remote == false`) → allow
3. Frontend in `allow-all` mode → allow
4. Sender (peer ID or channel address) in allowlist → allow
5. Default → deny

### Configuration

Config-store scope `sender-policy`:
- `allowlist-<frontend>` — comma-separated sender identifiers
- `mode-<frontend>` — `"deny"` (default) or `"allow-all"`

Managed via `/policies` TUI command (interactive add/remove/list/mode per frontend) or config-store writes. Policy cache has 30-second TTL in the gateway.

### Access Control

Gateway component has read access to `sender-policy` scope via `default_extra_read_scopes` in config-store policy. CLI writes as admin component (`harmonia-cli`).

## Security Kernel State

SignalGuard security kernel is active (v6). Core components:

### Typed Signal Dispatch

External signals from gateway are `harmonia-signal` structs (not format-strings). The conductor dispatches via `etypecase`:
- `harmonia-signal` → `orchestrate-signal` (boundary-wraps payload, sends to LLM, proposed tool commands pass policy gate)
- `string` → `orchestrate-prompt` (internal/TUI, may contain tool commands directly)

### Policy Gate

`%policy-gate` is a deterministic binary gate protecting 14 privileged operations:
- vault-set, vault-delete, config-set, harmony-policy-set, matrix-set-edge, matrix-set-node, matrix-reset-defaults, model-policy-upsert, model-policy-set-weight, codemode-run, git-commit, self-push, parallel-set-width, parallel-set-price

Gate logic:
- Non-privileged ops: always allowed (harmonic routing still applies).
- Privileged ops with tainted origin (`:external`, `:tool-output`, `:memory-recall`): **denied**.
- Privileged ops from non-owner/non-authenticated label: **denied**.
- Privileged ops from owner/authenticated + internal taint: **allowed**.

### Taint Propagation

`*current-originating-signal*` is a dynamic variable set by `orchestrate-signal` before LLM call. The policy gate reads it to determine the taint chain of the current reasoning context. When nil (internal/TUI prompt), owner trust is assumed.

### Safe Parsers

All `read-from-string` calls on external data replaced with:
- `%safe-parse-number`: validates `[0-9.eE+-]` only, binds `*read-eval* nil`, checks `realp`.
- `%safe-parse-policy-value`: rejects `#.` reader macros, validates safe types only.

### Vault Security

- Vault writes are key-strict and encrypted at rest with AES-256-GCM.
- Vault encryption root is derived from Harmoniis wallet slot family `vault` (legacy-compatible with `harmonia-vault`) first; explicit `HARMONIA_VAULT_MASTER_KEY` is fallback-only.
- Secret reads are component-scoped via `get_secret_for_component(component, symbol)` with default-deny behavior for unknown components.
- MQTT TLS lineage stores a deterministic `mqtt_tls_master_seed` derived from vault root material, and can persist client cert/key PEM in vault.

### Invariant Guards

Hardcoded non-configurable limits enforced by `%invariant-guard`:
- Vault min_harmony >= 0.30
- Dissonance-weight >= 0.05
- Cannot disable injection scanning
- Cannot enable `*read-eval*` on external data paths

### Security Posture Tracking

`*security-posture*` tracks system-wide security state:
- `:nominal` — no significant anomalies
- `:elevated` — moderate injection activity detected
- `:alert` — high injection activity, noise floors auto-adjusted

Updated by `:security-audit` phase in harmonic state machine.

### Adaptive Shell State

- Gateway signals carry `dissonance` score from inline injection scanning at parse time.
- Harmonic matrix supports `route_allowed_with_context` with `security_weight` and `dissonance` parameters.
- Search tool results (exa, brave) are boundary-wrapped before returning to conductor.
- Memory recalls are boundary-wrapped before prompt assembly.

### Security Config

`:security` section in `config/harmony-policy.sexp`:
- `dissonance-weight`: 0.15
- `anomaly-threshold-stddev`: 2.0
- `privileged-ops`: list of 14 gated operations
- `admin-intent-required-for`: operations requiring Ed25519 signature

## Erlang-Style Fault Tolerance (v7)

The core control loop never crashes. Every tick action is individually supervised.

### Supervision Architecture

- `%supervised-action` wraps every tick action in `handler-case` catching `serious-condition`. Errors are logged, recorded to the error ring, and the tick continues.
- Tick actions run inline — no intermediate list allocation per tick. Actions: `%tick-gateway-poll` → `%tick-tailnet-poll` → `%tick-actor-supervisor` → `%tick-process-prompt` → `%tick-actor-deliver` → `memory-heartbeat` → `harmonic-step` → `%tick-chronicle-flush` → `%tick-gateway-flush` → `%tick-tailnet-flush`.
- Outbound queue drain uses atomic swap (grab + clear) instead of copy-list + remove.
- Consecutive error tracking with adaptive cooldown: 5x sleep after 10 consecutive error ticks.
- Outer `handler-case` in `run-loop` — even if tick-level supervision somehow fails, the loop survives.

### Gateway Hardening

Frontend operations within `harmonia-runtime` are wrapped in `catch_unwind` — a panicking frontend cannot crash the runtime. Panic payloads are captured and returned as `Err(String)`. Per-frontend crash counts tracked via atomic counter.

### Runtime Self-Knowledge

`src/core/introspection.lisp` provides full runtime awareness:

- **Platform detection**: macOS, Linux, FreeBSD, Windows
- **Path introspection**: state root, source root, lib dir, log path, runtime dir
- **Library tracking**: module status and crash counts
- **Error ring**: circular 64-entry buffer of recent errors for self-diagnosis
- **Self-compilation**: `%cargo-build-component` runs cargo build for a single crate
- **Diagnostic snapshot**: `introspect-runtime` returns everything in one call
- **DNA integration**: `%runtime-self-knowledge` injects self-awareness into LLM system prompt

### Platform Path Structure

System artifacts separated from user data:

| Category | Path | Contents |
|---|---|---|
| User data | `~/.harmoniis/harmonia/` | vault.db, config.db, metrics.db, config/, frontends/, state/ |
| Libraries | `~/.local/lib/harmonia/` | runtime support files |
| App data | `~/.local/share/harmonia/` | Lisp source, docs, genesis, evolution knowledge |
| Binary | `~/.local/bin/harmonia` | CLI binary |
| Logs | `~/Library/Logs/Harmonia/` (macOS) | harmonia.log |
| Runtime | `$TMPDIR/harmonia/` (macOS) | phoenix.pid, runtime.sock (IPC socket, 0600 permissions) |

### Evolution Portability

Uninstall checks evolution safety before proceeding:
- Verifies source is committed and pushed to git remote
- Verifies binary evolution propagated to distributed store
- If local-only: strong warning, offers `evolution-export` backup, requires 100% confirmation
- `harmonia uninstall evolution-export` — portable tar.gz archive
- `harmonia uninstall evolution-import <archive> [--merge]` — restore into fresh install

## Chronicle Knowledge Base (v8)

The chronicle (`lib/core/chronicle`) is the agent's durable, queryable knowledge base — a graph-native SQLite store recording harmonic evolution, concept graph decompositions, delegation decisions, memory events, and recovery lifecycle.

### Database

SQLite WAL-mode at `{HARMONIA_STATE_ROOT}/chronicle.db`. Schema version tracked in `chronicle_meta` table with numbered migration functions.

### Tables (9)

| Table | Purpose | Retention |
|---|---|---|
| `harmonic_snapshots` | Full vitruvian triad + chaos + lorenz + lambdoma + security per cycle | Pressure-aware GC |
| `harmony_trajectory` | 5-minute downsampled buckets of signal evolution | Never pruned |
| `memory_events` | Crystallisation, compression, graph growth/pruning | Pressure-aware GC |
| `delegation_log` | Model selection: task, model, backend, cost, latency, tokens, success | Pressure-aware GC |
| `phoenix_events` | Supervisor lifecycle: start, child_exit, restart, max_restarts | Pressure-aware GC |
| `ouroboros_events` | Self-repair: crash, patch_write, patch_apply, recovery | Pressure-aware GC |
| `graph_snapshots` | S-expression concept graph with FNV-1a digest dedup | Pressure-aware GC |
| `graph_nodes` | Decomposed concept nodes (label, domain, depth) per snapshot | Cascades with snapshots |
| `graph_edges` | Decomposed concept edges (from, to, relation, weight) per snapshot | Cascades with snapshots |

### Concept Graph Traversal

Concept graphs from `memory-map-sexp` are decomposed into relational `graph_nodes` and `graph_edges` tables, enabling:

- **N-hop reachability**: `traverse_from(label, max_depth)` — recursive CTE over `graph_edges`.
- **Interdisciplinary bridges**: detects cross-domain concept connections.
- **Domain distribution**: concept count by domain for structural analysis.
- **Central concepts**: highest-connectivity concepts across graph history.
- **Graph evolution**: graph size trajectory over time (node/edge counts per snapshot).

### Arbitrary SQL Query

`(chronicle-query sql)` runs any SELECT/WITH SQL against the knowledge base, returning parsed s-expression results. Enables the agent to:

- Time-travel through harmonic evolution
- Analyze delegation cost/performance patterns
- Detect concept graph structural changes
- Correlate memory events with harmonic state
- Build complex queries with recursive CTEs, aggregation, window functions

### Pressure-Aware GC

Size-based pruning (not time-based):

| Tier | Threshold | Action |
|---|---|---|
| Soft | 50 MB | Thin old normal-signal data |
| Hard | 150 MB | Aggressive thinning, keep only inflection points |
| Critical | 300 MB | Emergency pruning of all but high-signal rows |

Inflection points preserved: high chaos_risk (> 0.7), rewrite_ready cycles, failed events, recovery events. `harmony_trajectory` is never pruned.

### Integration Points

- **Harmonic machine** (`:stabilize` phase): records full snapshot + concept graph decomposition.
- **Memory compression**: records crystallise/compress events with sizes and ratios.
- **Conductor**: records delegation decisions after each LLM call.
- **Phoenix**: records supervisor lifecycle via direct Rust API (rlib).
- **Ouroboros**: records crash/patch events via direct Rust API (rlib).
- **Boot**: `(init-chronicle-port)` initializes after evolution port.

### A2UI Dashboard

`chronicle-dashboard-json` generates an 8-panel A2UI Composite: harmony overview, phase progress, graph summary, trajectory table, delegation table, memory table, lifecycle table, cost summary.
