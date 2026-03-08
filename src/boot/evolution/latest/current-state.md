# Current State

Snapshot date: 2026-03-06

## Active Evolution Mode

Default mode is `:source-rewrite` (Ouroboros-backed patch flow).

From `src/ports/evolution.lisp`:

- `evolution-prepare` inspects health and crash state.
- `evolution-execute` writes patch artifacts in source-rewrite mode.
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

Model selection is task-aware and can prefer local CLI agents for software-dev prompts.

Policy inputs:

- `config/model-policy.sexp`
- `config/swarm.sexp`
- mutable state files under `HARMONIA_STATE_ROOT`.

## Memory Evolution State

Memory pipeline is active with four layers:

- Soul seeding from DNA,
- Daily interaction memory,
- Skill compression and crystallization,
- Temporal journaling (yesterday summary).

Compression and crystal thresholds are policy-controlled (`:memory` section in harmony policy).

## Gateway Signal Protocol State

Gateway signals now carry two enrichment layers:

- **Capabilities** (static per-frontend): parsed from `:capabilities` in `config/baseband.sexp` at registration time. Attached to every signal from that frontend. Example: `(:a2ui "1.0" :push "t")`.
- **Metadata** (dynamic per-message): emitted by the frontend as a third tab-field in poll output. Example: `(:platform "ios" :device-id "uuid-123" :a2ui-version "1.0")`.

Poll format is now 3-field backward-compatible: `sub_channel\tpayload[\tmetadata]`.

A2UI dispatch is capabilities-driven — the conductor checks signal capabilities, not frontend names. Any frontend declaring `:capabilities (:a2ui "1.0")` gets A2UI context injection and component catalog availability.

A2UI component catalog: `config/a2ui-catalog.sexp` (21 components, lazily loaded and cached by conductor).

Push notifications: `lib/frontends/push` is an `rlib` utility consumed by mqtt-client for offline device push via HTTP webhook.

## Matrix Enforcement State

All critical orchestrator routes are matrix-gated before invocation.

Matrix topology source of truth:

- seed: `config/matrix-topology.sexp` (includes gateway node)
- mutable state: `${HARMONIA_STATE_ROOT}/matrix-topology.sexp`

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
