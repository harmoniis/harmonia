# Rewrite Roadmap

This roadmap defines high-leverage evolution targets while preserving genesis alignment.

## Priority 1: Stability And Observability

- [DONE v7] Erlang-style supervision: every tick action wrapped in `%supervised-action`, never crashes.
- [DONE v7] Error ring buffer (64 entries) for self-diagnosis via `introspect-recent-errors`.
- [DONE v7] Library crash tracking with per-library crash counts, status, timestamps.
- [DONE v7] `catch_unwind` on all gateway calls — panicking frontends cannot crash the runtime.
- [DONE v7] Adaptive cooldown: 5x sleep after 10 consecutive error ticks.
- [DONE v7] Runtime self-knowledge injected into DNA system prompt for autonomous debugging.
- [DONE v8] Chronicle knowledge base: graph-native SQLite store with 9 tables for harmonic, memory, delegation, recovery, and concept graph data.
- [DONE v8] Arbitrary SQL query (`chronicle-query sql`) returning s-expression results for agent self-reasoning.
- [DONE v8] Concept graph decomposition into relational tables with recursive CTE traversal.
- [DONE v8] Pressure-aware GC: size-based pruning preserving high-signal inflection points.
- [DONE v8] Harmony trajectory: permanently downsampled 5-minute buckets, never pruned.
- [DONE v8] A2UI dashboard: 8-panel Composite for harmony, delegation, memory, lifecycle visualization.
- Extend matrix time-series usage in runtime decisions (chronicle data available for this).
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

- [DONE v7] Evolution export/import: `harmonia uninstall evolution-export` / `evolution-import --merge`.
- [DONE v7] Uninstall safety gate: checks git push status and distributed propagation before allowing removal.
- [DONE v7] Self-compilation: `%cargo-build-component` builds individual crates from within the agent.
- Standardize rewrite preflight checks.
- Require explicit rollback plans in automated patch proposals.
- Tie rewrite acceptance to measurable score delta, not only successful execution.

## Priority 5: Signal And Channel Maturity

- [DONE v5] Capabilities-driven A2UI dispatch (no hardcoded frontend names).
- [DONE v5] Gateway signal metadata and capabilities enrichment.
- [DONE v5] Push notification integration as rlib utility.
- [DONE v5] MQTT device registry with offline queue and push.
- [DONE v5] A2UI component catalog (`config/a2ui-catalog.sexp`).
- [DONE v10] Unified command dispatch: gateway as single interception point for all /commands from all frontends.
- [DONE v10] All cdylib crate-types removed. Frontends are rlib crates compiled into `harmonia-runtime`.
- [DONE v11] FFI layer fully removed. SBCL communicates with Rust via IPC (Unix domain socket).
- [DONE v11] `harmonia-runtime` crate (`lib/core/runtime/`): single Rust binary with all ractor actors.
- [DONE v11] Phoenix supervisor (`lib/core/phoenix/`): ractor-based multi-subsystem process supervisor with health endpoint.
- [DONE v11] CLI lifecycle: `harmonia start/stop/restart/status` managing Phoenix.
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

## Priority 7: Platform Maturity

- [DONE v7] Platform-correct path structure: user data separate from system artifacts.
- [DONE v7] XDG-style paths: `~/.local/lib/harmonia/`, `~/.local/share/harmonia/`, `~/.local/bin/`.
- [DONE v7] Platform-specific runtime dirs: macOS `$TMPDIR`, Linux `$XDG_RUNTIME_DIR`.
- [DONE v7] Platform-specific log dirs: macOS `~/Library/Logs/Harmonia/`, Linux `~/.local/state/harmonia/`.
- [DONE v7] Library path fallback chain with env override support.
- [DONE v11] Single-binary Rust runtime eliminates shared library deployment complexity.
- Windows support testing (paths defined but untested).
- FreeBSD service file (rc.d script template exists but untested).

## Scope Exclusions

Avoid rewrites that:

- bypass matrix route checks,
- expose vault values in memory/logging,
- weaken security kernel invariant guards,
- allow `*read-eval*` on external data paths,
- bypass policy gate for privileged operations,
- remove taint propagation from signal dispatch,
- or weaken DNA validation at startup.
