# Rewrite Roadmap

This roadmap defines high-leverage evolution targets while preserving genesis alignment.

## Priority 1: Stability And Observability

- Extend matrix time-series usage in runtime decisions.
- Improve structured error taxonomy and recovery summaries.
- Keep boot-to-ready path deterministic under partial frontend availability.

## Priority 2: Token And Tool Efficiency

- Expand codemode pipelines for multi-step deterministic operations.
- Reduce unnecessary LLM relay when tool plans are predictable.
- Use policy feedback loops to tune model selection for task classes.

## Priority 3: Memory Quality

- Improve crystallization heuristics for decision-heavy sessions.
- Increase topic coherence in temporal journal generation.
- Evaluate edge quality in concept graph for better rewrite focus.

## Priority 4: Evolution Safety

- Standardize rewrite preflight checks.
- Require explicit rollback plans in automated patch proposals.
- Tie rewrite acceptance to measurable score delta, not only successful execution.

## Priority 5: Signal And Channel Maturity

- [DONE v5] Capabilities-driven A2UI dispatch (no hardcoded frontend names).
- [DONE v5] Gateway signal metadata and capabilities enrichment.
- [DONE v5] Push notification integration as rlib utility.
- [DONE v5] MQTT device registry with offline queue and push.
- [DONE v5] A2UI component catalog (`config/a2ui-catalog.sexp`).
- Extend capabilities model to non-A2UI features (e.g., `:voice`, `:location`, `:accessibility`).
- Frontend capability negotiation at connect time (dynamic capability update).
- A2UI catalog versioning for forward/backward component compatibility.

## Priority 6: Security Hardening

- [DONE v6] SignalGuard security kernel: typed signals, policy gate, taint propagation.
- [DONE v6] Safe parsers replacing all `read-from-string` on external data.
- [DONE v6] Boundary wrapping for external data in prompts, memory, search results.
- [DONE v6] Matrix threshold hardening for privileged edges.
- [DONE v6] Gateway dissonance scanning at signal parse time.
- [DONE v6] Security-aware harmonic routing (`route_allowed_with_context`).
- [DONE v6] `:security-audit` phase in harmonic state machine.
- [DONE v6] Tailnet HMAC authentication and replay protection.
- [DONE v6] MQTT fingerprint validation.
- [DONE v6] Vault encryption at rest with audit logging.
- [DONE v6] `signal-integrity` shared crate for injection detection.
- [DONE v6] `admin-intent` crate for Ed25519 signed admin intents.
- Tamper-evident security ledger (append-only hash-chained event log).
- Behavioral baseline + anomaly detection (rolling per-frontend statistics).
- Daily security digest generation alongside memory journaling.
- Frontend capability negotiation for dynamic security label upgrade.

## Scope Exclusions

Avoid rewrites that:

- bypass matrix route checks,
- expose vault values in memory/logging,
- weaken security kernel invariant guards,
- allow `*read-eval*` on external data paths,
- bypass policy gate for privileged operations,
- remove taint propagation from signal dispatch,
- or weaken DNA validation at startup.
