# Changelog

Append-only evolution ledger.

## v0 — Genesis

- Human-authored bootstrap corpus and runtime skeleton.
- DNA constitution anchored.
- Core loop and CFFI orchestration path established.

## v1 — 2026-02-17

- Runtime orchestration scaffold stabilized.
- Uniform C-ABI exports standardized across many crates.
- Shared linkage path enabled across platforms.

## v2 — 2026-02-18

- Harmonic matrix and runtime policy moved toward data-driven control.
- Hardcoded operational policy reduced in favor of `.sexp` configuration.

## v3 — 2026-03-05

- Parallel-agents gained tmux-driven CLI swarm tier.
- Multi-agent orchestration expanded beyond API-only subagents.
- Rewrite execution protocol strengthened with CLI automation capabilities.

## v4 — 2026-03-05

- Architecture formalized into core/backends/tools/frontends pillars.
- Gateway/baseband channel became central frontend dispatch path.
- Tailnet/tailscale channel integrated into mesh-ready communication model.

## v5 — 2026-03-06

- Target: A2UI signal protocol, capabilities-driven routing, push integration.
- Motivation: Eliminate hardcoded frontend-name checks; make A2UI generic across any frontend.
- Law/Principle Applied: Boundary-first safety; compression as intelligence pressure (capabilities in config, not code).
- Changes:
  - Gateway Signal carries `metadata` (per-message) and `capabilities` (per-frontend from baseband config).
  - Poll format extended to 3-field backward-compatible: `sub_channel\tpayload[\tmetadata]`.
  - FrontendHandle stores parsed capabilities from `:capabilities (...)` in config sexp.
  - Conductor checks signal capabilities (not frontend name) for A2UI dispatch.
  - A2UI component catalog (`config/a2ui-catalog.sexp`) — 21 components injected into LLM context.
  - Push-sns replaced with generic push rlib (`lib/frontends/push`) consumed by mqtt-client.
  - MQTT frontend gained device registry, offline queue, and push notification integration.
  - Loop.lisp fixed: `:frontend`/`:sub-channel` properly extracted from nested `:channel` plist.
  - Gateway node added to matrix topology.
  - Text fallback extraction for A2UI payloads sent to non-A2UI frontends.
- Risk Notes: 3-field poll format is backward compatible; 2-field frontends unaffected.
- Rollback Plan: Revert capabilities/metadata fields to None; restore 2-field parser.

## v6 — 2026-03-07

- Target: SignalGuard — Security Kernel + Adaptive Harmonic Shell.
- Motivation: Close critical signal injection, arbitrary code execution, and confused deputy vulnerabilities.
- Law/Principle Applied: Boundary-first safety; LLM output is a proposal, not a command; deterministic gates for privileged ops.
- Changes:
  - **Security Kernel (deterministic, non-bypassable)**:
    - `harmonia-signal` struct replaces format-string prompts for external signals (typed signals end-to-end).
    - `%policy-gate` — deterministic binary gate for 14 privileged operations (vault-set, config-set, harmony-policy-set, matrix-set-edge, etc.). Checks taint chain and security label. Blocks tainted external/tool-output/memory-recall origins.
    - `*current-originating-signal*` dynamic variable propagates taint through orchestration chain. Set during `orchestrate-signal`, nil during `orchestrate-prompt` (owner trust).
    - Split dispatch: `orchestrate-once` dispatches to `orchestrate-signal` (external, never tool-parses payload) or `orchestrate-prompt` (internal, may contain tool commands).
    - All 12+ `read-from-string` calls on external data replaced with `%safe-parse-number` and `%safe-parse-policy-value` (no Lisp reader macros).
    - `*read-eval*` bound to nil at every remaining `read-from-string` site.
    - `%invariant-guard` — hardcoded non-configurable safety limits (vault min_harmony >= 0.30, dissonance-weight >= 0.05).
  - **Adaptive Shell (harmonic, self-tuning)**:
    - Gateway `Signal` struct carries `dissonance: f64` from injection scanning at parse time.
    - `signal-integrity` crate — shared injection detection + dissonance scoring (extended patterns: social engineering, Lisp reader macros, Harmonia-specific tool injection).
    - `route_allowed_with_context` in harmonic-matrix — security-aware routing with `security_weight` and `dissonance` parameters.
    - `:security-audit` phase added to harmonic state machine (observe injection counts, update posture, auto-adjust noise floors).
    - `*security-posture*` tracking (`:nominal`/`:elevated`/`:alert`).
  - **Boundary Wrapping**: External data wrapped with `=== EXTERNAL DATA [...] ===` markers in prompt assembly, memory recall, and search tool results (search-exa, search-brave).
  - **Matrix Hardening**: Raised min_harmony on privileged edges — vault `0.10→0.70`, harmonic-matrix `0.10→0.60`, git-ops `0.20→0.55`.
  - **Tailnet HMAC Auth**: `MeshMessage` carries `timestamp_ms` + `hmac` (HMAC-SHA256). 5-minute replay window. Shared secret from env var.
  - **MQTT Fingerprint Validation**: `validate_agent_fingerprint` compares `agent_fp` against vault-stored expected fingerprint.
  - **Vault Encryption at Rest**: Values encrypted with AES-256-GCM, rooted in wallet slot family `vault` (legacy-compatible with `harmonia-vault`) first; explicit `HARMONIA_VAULT_MASTER_KEY` is fallback-only. Component-scoped read policy enabled.
  - **Admin Intent Crate**: Ed25519 signature verification for privileged mutations (`lib/core/admin-intent`).
  - **Config**: `:security` section added to `config/harmony-policy.sexp` with privileged-ops list, dissonance-weight, admin-intent-required-for.
- New Crates: `lib/core/signal-integrity`, `lib/core/admin-intent`.
- Risk Notes: Typed signal dispatch is backward compatible — string prompts still handled by `orchestrate-prompt`. `#[serde(default)]` on new MeshMessage fields preserves tailnet backward compatibility.
- Rollback Plan: Revert `orchestrate-once` to string-only dispatch; remove policy gate calls; restore format-string gateway-inbound prompts.

## v7 — 2026-03-10

- Target: Erlang-style fault tolerance, runtime self-knowledge, platform-correct paths, evolution portability.
- Motivation: The agent must never crash. Dynamic libraries may crash and be reloaded, but the core loop survives and gracefully degrades. The agent must know itself — platform, paths, libraries, errors — and repair itself autonomously. System artifacts (binaries, libraries, source) must not pollute user data space.
- Law/Principle Applied: Rule 7 (Never crash — gracefully degrade), Rule 8 (Know thyself — understand your own runtime and how to repair it). XDG/platform conventions for file placement.
- Changes:
  - **Erlang-Style Supervision** (`src/core/loop.lisp`):
    - Every tick action wrapped in `%supervised-action` — catches `serious-condition`, records to error ring, never propagates.
    - Tick actions run inline (no intermediate list allocation per tick).
    - Outbound queue uses atomic swap: grab-and-clear instead of `copy-list` + quadratic `remove`.
    - Inbound signal enqueue uses `nconc` instead of `append` (avoids copying entire queue).
    - Consecutive error tracking with adaptive cooldown (5x sleep after 10 error ticks).
    - Outer `handler-case` in `run-loop` — the loop itself truly never crashes.
    - `declaim inline` hint on `%supervised-action` for SBCL optimization.
  - **Gateway FFI Hardening** (`lib/core/gateway/`):
    - All unsafe FFI calls in `frontend_ffi.rs` wrapped in `std::panic::catch_unwind(AssertUnwindSafe(...))` — panicking cdylibs cannot crash the gateway process.
    - `FrontendVtable::reload()` — shutdown, drop library, reload from disk, re-init with stored config.
    - `Registry::reload(name)` — unregister + re-register preserving crash count.
    - `Registry::crash_count(name)` — atomic per-frontend crash counter.
    - New FFI exports: `harmonia_gateway_reload`, `harmonia_gateway_crash_count`.
  - **Runtime Introspection** (`src/core/introspection.lisp` — NEW):
    - Platform detection: `%platform`, `%platform-name` (macOS/Linux/FreeBSD/Windows).
    - Path introspection: `%state-root`, `%source-root`, `%lib-dir`, `%log-path`, `%runtime-dir`.
    - Library tracking: `*loaded-libs*` hash table with crash counts, timestamps, status.
    - Error ring buffer: circular 64-entry `*error-ring*` for self-diagnosis.
    - Self-compilation: `%cargo-build-component` runs `cargo build --release -p <crate>`.
    - Hot-reload: `%hot-reload-frontend` rebuilds crate, copies dylib, unregisters + re-registers.
    - Full diagnostic: `introspect-runtime` returns complete snapshot for LLM reasoning.
    - `%runtime-self-knowledge` injects platform/path/repair capabilities into DNA system prompt.
  - **DNA Rules 7 & 8** (`src/dna/dna.lisp`):
    - Rule 7: "Never crash — gracefully degrade. Catch errors, record them, reload failed components."
    - Rule 8: "Know thyself — understand your own runtime, logs, libraries, and how to repair them."
    - System prompt now includes runtime self-knowledge block (platform, paths, libraries, self-repair guide).
  - **Baseband Port** (`src/ports/baseband.lisp`):
    - CFFI bindings for `harmonia_gateway_reload` and `harmonia_gateway_crash_count`.
    - Lisp wrappers: `gateway-reload`, `gateway-crash-count`.
    - `gateway-register` now calls `%register-loaded-lib` for library tracking.
  - **Platform-Correct Path Structure** (`cli/paths.rs`):
    - `~/.harmoniis/harmonia/` — user data ONLY (databases, config, state, frontends).
    - `~/.local/lib/harmonia/` — cdylibs (platform-standard XDG location).
    - `~/.local/share/harmonia/` — source, docs, genesis (platform-standard XDG location).
    - `~/.local/bin/harmonia` — binary (direct copy, not symlink).
    - `~/Library/Logs/Harmonia/` (macOS) / `~/.local/state/harmonia/` (Linux) — logs.
    - `$TMPDIR/harmonia/` (macOS) / `$XDG_RUNTIME_DIR/harmonia/` (Linux) — PID, socket.
    - Library path fallback chain: `HARMONIA_LIB_DIR` env → `target/release/` → `~/.local/lib/harmonia/`.
  - **Uninstall with Evolution Safety** (`cli/uninstall.rs`):
    - Detects evolution state and checks if source is pushed to git and/or propagated to distributed store.
    - If local-only evolution: strong warning, offers `evolution-export` before proceeding, requires explicit confirmation.
    - New subcommands: `harmonia uninstall evolution-export [-o path.tar.gz]`, `harmonia uninstall evolution-import <archive> [--merge]`.
    - Export creates portable tar.gz with evolution versions, genesis knowledge, config keys, manifest.
    - Import supports replace mode (default) and `--merge` mode (additive version merge, higher version wins).
    - Uninstall removes: libs, source, binary, logs, runtime, evolution state, shell rc blocks, system service.
    - Uninstall preserves: vault.db, config.db, metrics.db, config/, frontends/, state/.
  - **Setup/Start Updated** (`cli/setup.rs`, `cli/start.rs`):
    - Setup installs libs to `~/.local/lib/harmonia/`, source to `~/.local/share/harmonia/`, binary to `~/.local/bin/`.
    - Start resolves lib dir from config-store → platform lib dir → target/release (dev fallback).
    - Start resolves source dir from config-store → cwd → share dir → legacy fallback.
- Risk Notes: Platform path migration is backward-compatible — legacy `~/.harmoniis/harmonia/lib/` and `~/.harmoniis/harmonia/src/` are checked as fallbacks. `handler-case` is zero-cost on SBCL happy path (setjmp/longjmp). `catch_unwind` adds minimal overhead only when a panic actually occurs.
- Rollback Plan: Revert paths.rs to data_dir()-relative lib/src; revert loop.lisp to list-based tick planner; remove introspection.lisp from boot sequence; remove catch_unwind wrappers from frontend_ffi.rs.

## v8 — 2026-03-10

- Target: Chronicle — Graph-native knowledge base and time-series observability.
- Motivation: The agent must durably record and reason over its own evolution. Harmonic snapshots, memory events, delegation decisions, concept graphs, and recovery events should be SQL-queryable. The agent should be able to time-travel through its own history, traverse concept graph evolution, and analyze delegation cost/performance via complex SQL returning s-expressions.
- Law/Principle Applied: Compression as intelligence pressure (concept graphs decomposed into relational tables); attractor-seeking runtime (trajectory data enables convergence analysis); know thyself (queryable self-knowledge).
- Changes:
  - **Chronicle Crate** (`lib/core/chronicle/` — NEW, cdylib + rlib):
    - SQLite WAL-mode knowledge base at `{HARMONIA_STATE_ROOT}/chronicle.db`.
    - Schema migration system via `chronicle_meta` table with version tracking.
    - 9 tables: `harmonic_snapshots`, `memory_events`, `phoenix_events`, `ouroboros_events`, `delegation_log`, `harmony_trajectory` (5-min downsampled buckets, never pruned), `graph_snapshots`, `graph_nodes`, `graph_edges`.
    - `query_sexp(sql)` — arbitrary SELECT/WITH SQL returning parsed s-expression results. Enables the agent to reason over its own history with full SQL power (recursive CTEs, aggregation, window functions).
    - `record_graph_snapshot()` — decomposes s-expression concept graphs into relational `graph_nodes` and `graph_edges` tables with FNV-1a digest deduplication.
    - `traverse_from(label, max_depth)` — recursive CTE for N-hop graph reachability.
    - `interdisciplinary_bridges()` — detects cross-domain concept connections.
    - `domain_distribution()` — concept count by domain for structural analysis.
    - `central_concepts(limit)` — highest-connectivity concepts across graph history.
    - `graph_evolution(since, limit)` — graph size trajectory over time.
  - **Pressure-Aware GC** (`db.rs`):
    - Size-based pruning: soft (50MB), hard (150MB), critical (300MB) tiers.
    - Signal-preserving: keeps inflection points (high chaos_risk > 0.7, rewrite_ready, failed events) while thinning normal data proportionally.
    - `harmony_trajectory` is never pruned — permanently downsampled, negligible storage.
    - `gc_status()` returns current DB size and pressure tier as s-expression.
  - **Chronicle Port** (`src/ports/chronicle.lisp` — NEW):
    - 40+ CFFI bindings following evolution.lisp pattern.
    - `chronicle-record-harmonic` extracts all 24 values from harmonic context plist.
    - `chronicle-record-graph-snapshot` decomposes concept graph (from `memory-map-sexp`) into JSON arrays for relational storage.
    - Query API returns parsed s-expressions via `read-from-string`.
  - **Harmonic Machine Integration** (`src/core/harmonic-machine.lisp`):
    - `:stabilize` phase records full harmonic snapshot and concept graph decomposition to chronicle.
  - **Memory Integration** (`src/memory/store/compression.lisp`):
    - Crystallisation and compression events recorded with entry counts, sizes, compression ratios.
  - **Conductor Integration** (`src/orchestrator/conductor.lisp`):
    - Delegation decisions recorded after each LLM call: task_hint, model, backend, cost, latency, tokens, escalation, success.
  - **Phoenix Integration** (`lib/core/phoenix/src/main.rs`):
    - Direct Rust API (rlib) — records startup, child_exit, restart, max_restarts events.
  - **Ouroboros Integration** (`lib/core/ouroboros/src/lib.rs`):
    - Direct Rust API (rlib) — records crash and patch_write events.
  - **A2UI Dashboard** (`dashboard.rs`):
    - `chronicle_dashboard_json()` returns 8-panel Composite: harmony overview, phase progress, graph summary, trajectory table, delegation table, memory table, lifecycle table, cost summary.
  - **Boot Integration** (`src/core/boot.lisp`):
    - `(init-chronicle-port)` added to boot sequence after evolution port. 16 chronicle symbols exported.
- New Crate: `lib/core/chronicle`.
- Risk Notes: Chronicle is append-only by default. All integration points use `ignore-errors` — chronicle failure cannot affect core orchestration. Phoenix/Ouroboros use rlib (no FFI overhead). Pressure-aware GC preserves high-signal data even under aggressive pruning.
- Rollback Plan: Remove `(init-chronicle-port)` from boot.lisp; remove `ignore-errors` chronicle calls from harmonic-machine, compression, conductor; remove chronicle from Phoenix/Ouroboros Cargo.toml dependencies.

## v9 — 2026-03-10

- Target: Provider-scoped seed evolution, swarm-first orchestration, and operator-visible seed setup controls.
- Motivation: Avoid provider bias defaults, start from low-cost/high-throughput seed models, and keep orchestrator focused on planning/delegation instead of single-backend execution.
- Law/Principle Applied: Harmonic optimization via explicit price/speed/success/reasoning/vitruvian weighting with auditable state.
- Changes:
  - **Seed Policy Source of Truth** (`src/core/model-policy.lisp`, `config/model-policy.sexp`):
    - Provider-aware seed resolution chain: `seed-models-<provider>` (config-store) → provider defaults in policy → global `seed-models` override.
    - Active provider resolved from config-store (`model-policy/provider`) before policy fallback.
    - OpenRouter default seed set to:
      1. `inception/mercury-2`
      2. `qwen/qwen3.5-flash-02-23`
      3. `minimax/minimax-m2.5`
      4. `google/gemini-3.1-flash-lite-preview`
    - Seed evolution scoring weighted by price/speed/success/reasoning/vitruvian with sample threshold gating.
  - **CLI-First + Cooloff** (`src/core/model-policy.lisp`, `src/ports/swarm.lisp`):
    - CLI preference chain (`claude-code`, `codex`) remains first for configured task kinds.
    - Per-invocation CLI timeout (`:cli-timeout-seconds`) prevents "thinking" stalls from blocking swarm completion.
    - Quota/cooloff detection marks CLI candidates temporarily unavailable before falling back to API models.
  - **Orchestrator Delegation Model** (`src/orchestrator/conductor.lisp`):
    - Non-tool LLM work delegates through swarm (`parallel-solve`) rather than direct single-model backend loops.
    - Optional large-context summarization step introduced before delegation, controlled by model-policy keys.
  - **Structured Swarm Outcomes** (`src/ports/swarm.lisp`):
    - Structured result parsing records model, success, latency, and cost for each subagent.
    - Outcome metrics are recorded into model-policy experience history and chronicle delegation telemetry.
  - **Setup UX** (`cli/main.rs`, `cli/setup.rs`):
    - Added `harmonia setup --seeds` for seed-policy-only updates.
    - Setup writes provider-scoped default seed keys and active provider seed overrides into config-store.
- Risk Notes: Swarm-first delegation increases dependence on parallel-agents route availability; summarization pre-pass may alter wording on large-context tasks.
- Rollback Plan: Re-enable direct conductor backend completion path, remove seeds-only setup branch, and fall back to policy-only seed list resolution.

## Next Entry Template

Use this structure for the next evolution record:

```md
## vN — YYYY-MM-DD

- Target:
- Motivation:
- Law/Principle Applied:
- Score Before:
- Score After:
- Risk Notes:
- Rollback Plan:
```
