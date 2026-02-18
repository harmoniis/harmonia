# Session Report — 2026-02-18

## Scope Completed

This session implemented the core harmonic routing matrix, moved search into core runtime paths, linked orchestration + subagents directly to web verification, and updated testing/documentation.

## Implemented

1. Core `harmonic-matrix` module (Rust)
- Added `lib/core/harmonic-matrix/Cargo.toml`
- Added `lib/core/harmonic-matrix/BUILD.bazel`
- Added `lib/core/harmonic-matrix/src/lib.rs`
- C-ABI surface:
  - `harmonia_harmonic_matrix_init`
  - `harmonia_harmonic_matrix_register_node`
  - `harmonia_harmonic_matrix_set_tool_enabled`
  - `harmonia_harmonic_matrix_register_edge`
  - `harmonia_harmonic_matrix_route_allowed`
  - `harmonia_harmonic_matrix_observe_route`
  - `harmonia_harmonic_matrix_report`
  - `harmonia_harmonic_matrix_last_error`
  - `harmonia_harmonic_matrix_free_string`
- Behavior:
  - explicit node/edge registration,
  - harmonic threshold checks (`signal`, `noise`, edge weight, min-harmony),
  - hot-plug gating for optional tool nodes,
  - route telemetry (uses/success/latency/cost).

2. Lisp backend for matrix orchestration
- Added `src/backends/harmonic-matrix.lisp`.
- Added matrix bootstrapping to `src/core/boot.lisp`.
- Registered default topology (orchestrator/core/backend/memory/tool routes).
- Exported runtime operations:
  - `harmonic-matrix-set-tool-enabled`
  - `harmonic-matrix-report`

3. Search is now in core execution path
- Search crates already migrated to:
  - `lib/core/search-exa`
  - `lib/core/search-brave`
- `src/backends/integrations.lisp` now enforces matrix routes for search and records route observations for exa/brave + memory path.

4. Conductor route enforcement via matrix
- Updated `src/orchestrator/conductor.lisp` to require allowed routes before:
  - OpenRouter calls,
  - communication tools,
  - voice tools,
  - parallel subagent calls,
  - matrix admin/report calls.
- Added matrix tool ops:
  - `tool op=matrix-tool-enable tool=<id> enabled=<1|0|t|nil>`
  - `tool op=matrix-report`
- Added route observation from orchestrator -> used tool and used tool -> memory.

5. Parallel subagents now perform direct search verification
- Updated `lib/core/parallel-agents/src/lib.rs`.
- Each successful subagent completion now attempts verification using vault-backed keys directly:
  - `exa_api_key` via Exa,
  - fallback `brave_api_key` via Brave.
- Added verification fields per task:
  - `verified`, `verification_source`, `verification_detail`.
- Extended metrics TSV format and report aggregation to include verified-rate globally and per model.

6. Build/test wiring updates
- Updated `scripts/test-communication-tools.sh`:
  - builds `harmonia-harmonic-matrix`,
  - loads matrix dylib,
  - includes matrix healthcheck in CFFI smoke output.
- Updated root `BUILD` aggregation to include split tool crates and new core modules.
- Updated `doc/evolution/TOOLS.md` to reflect split tool architecture + new core modules.

7. Vault harmony fix (de-hardcoded secret onboarding)
- Removed per-key vault env hardcoding for Exa/Brave.
- Added generic env ingest path in vault:
  - `HARMONIA_VAULT_SECRET__<SYMBOL>=<VALUE>` auto-loads into vault without code changes.
- Added runtime orchestration interface:
  - `tool op=vault-set key=<symbol> value=<secret>`
- Routed vault writes through harmonic matrix (`orchestrator -> vault -> memory` telemetry).

8. Matrix hardcoding removal (set/get/persist interfaces)
- Matrix topology is now dynamic and persisted at:
  - `HARMONIA_MATRIX_TOPOLOGY_PATH` (default `/tmp/harmonia/matrix-topology.sexp`)
- Added runtime matrix control ops:
  - `tool op=matrix-set-node id=<node> kind=<core|backend|tool>`
  - `tool op=matrix-set-edge from=<id> to=<id> weight=<float> min=<float>`
  - `tool op=matrix-tool-enable tool=<id> enabled=<1|0|t|nil>`
  - `tool op=matrix-get-topology`
  - `tool op=matrix-route-check from=<id> to=<id> signal=<float> noise=<float>`
  - `tool op=matrix-save`
  - `tool op=matrix-load`
  - `tool op=matrix-reset-defaults`

9. Additional hardcoded knobs converted to set/get
- Parallel subagent width is now runtime-settable:
  - `tool op=parallel-set-width count=<int>`
  - `tool op=parallel-get-width`
- Model policy is now runtime-settable/gettable:
  - `tool op=model-policy-get`
  - `tool op=model-policy-set-weight metric=<completion|correctness|speed|price> value=<float>`
  - `tool op=model-policy-upsert id=<model-id> [tier=<keyword>] [cost=<n>] [latency=<n>] [quality=<n>] [completion=<n>] [tags=tag1,tag2]`

10. Data-driven config layer (hardcoded policy removed from execution paths)
- Added declarative config files:
  - `config/tools.sexp`
  - `config/model-policy.sexp`
  - `config/matrix-topology.sexp`
  - `config/parallel-policy.sexp`
  - `config/harmony-policy.sexp`
- Boot now loads policy/tool defaults from config files.
- Runtime updates persist to mutable state files (`/tmp/harmonia/*.sexp`) via save/load ops.
- Route default signal/noise now has runtime defaults and set/get API.
- Parallel policy now supports save/load:
  - `tool op=parallel-save-policy`
  - `tool op=parallel-load-policy`

12. 4D matrix upgrade (time as first-class dimension)
- Matrix core now tracks temporal route samples (`route_history`) and temporal events (`events`) with timestamps and revision.
- Added matrix time APIs:
  - `tool op=matrix-time-report since=<unix>`
  - `tool op=matrix-route-timeseries from=<id> to=<id> limit=<n>`
- Orchestrator now feeds matrix with input/output/error events continuously for system-wide observability.

13. Harmony-policy externalization
- Added runtime-loadable harmony constants file:
  - `config/harmony-policy.sexp`
- Replaced hardcoded rewrite/chaos/lorenz/lambdoma/vitruvian thresholds with policy lookups.
- Added harmony policy runtime ops:
  - `tool op=harmony-policy-get`
  - `tool op=harmony-policy-set path=<a/b/c> value=<lisp-literal>`
  - `tool op=harmony-policy-save`
  - `tool op=harmony-policy-load`

11. Documentation reconciliation (genesis + evolution)
- Added `doc/genesis/CODE_HARMONY.md` with explicit strength/utility/beauty coding constitution and internet-sourced references.
- Updated `doc/genesis/INDEX.md` and `doc/README.md` navigation to include new code harmony + runtime policy surfaces.
- Updated stale plugin references (`social`) to current split plugin architecture across genesis docs.
- Updated `doc/genesis/ARCHITECTURE.md` config sections to reflect real runtime config files and runtime set/get/save/load policy operations.
- Updated `doc/genesis/CONTEXT.md` and `doc/genesis/GENESIS_DEV_FLOW.md` to align with current config/runtime topology.

## Test Evidence (This Session)

1. Rust workspace tests
- Command: `cargo test --workspace`
- Result: pass (including new `harmonia-harmonic-matrix` and modified `harmonia-parallel-agents`).

2. Communication/search/voice smoke
- Command: `./scripts/test-communication-tools.sh`
- Result: pass.
- Healthchecks include matrix module.

3. Lisp runtime bootstrap + matrix report
- Command: SBCL boot + `tool op=matrix-report`
- Result: pass; matrix topology loaded and queryable from agent loop.

4. Lisp runtime + parallel report
- Command: SBCL boot + `tool op=parallel-report`
- Result: pass; report includes `:verified-rate`.

5. Live online execution (outside sandbox DNS limits)
- Command: SBCL boot + `tool op=search` + `tool op=parallel-solve` + `tool op=matrix-report`
- Result: OpenRouter online path succeeds when run outside sandbox DNS restrictions.
- Remaining limitation: Exa/Brave verification keys were not present in `.env` during this run, so verification reported `missing-search-keys`.

6. Hardproof grind
- `scripts/grind-harmonia-online.sh`: pass (OpenRouter + AWS identity + S3 upload).
- `scripts/test-mqtt-pgp-tls-local.sh`: pass (local broker, TLS chain, PGP binding, publish/poll).
- `scripts/test-core-live.sh`: pass (`http`, `recovery`, `fs`, `browser`, `cron` live checks).
- Harmonia self-push loop: pass (remote branch created and cleaned).
- `scripts/test-harmonic-genesis-loop.sh`: pass (`GENESIS_LOOP_OK`).

## Current State

- Harmonic matrix routing is now an active core constraint around orchestration paths.
- Search is treated as core infrastructure and used directly for subagent verification.
- Optional tools are hot-pluggable through matrix gating.
- Route and verification metrics are now part of observable system telemetry.

## Additional Update (Same Session Continuation)

14. Runtime-selectable 4D matrix storage backends
- `lib/core/harmonic-matrix` now supports runtime backend selection:
  - `memory` store
  - `sqlite` store
- New C-ABI:
  - `harmonia_harmonic_matrix_set_store(kind, path)`
- Persistence scope for `sqlite`:
  - nodes, tools, edges, route_samples, events, epoch/revision.
- Report/time-report now include:
  - `:store-kind`
  - `:store-path`
- Orchestrator tool op added:
  - `tool op=matrix-set-store kind=<memory|sqlite> [path=<db-path>]`

15. Verified sqlite persistence roundtrip
- Low-level CFFI roundtrip:
  - set store to sqlite -> mutate graph -> observe route/event -> re-init -> state preserved.
- Runtime agent command path verified:
  - `tool op=matrix-set-store kind=sqlite path=...` returns `MATRIX_STORE_SET`.

16. Matrix crate modularization
- Split former monolithic `lib/core/harmonic-matrix/src/lib.rs` into:
  - `src/model.rs` (state/data models)
  - `src/runtime.rs` (matrix logic + storage adapters)
  - `src/ffi.rs` (C-ABI boundary only)
  - `src/lib.rs` (module wiring only)
- Added deterministic Rust test:
  - `sqlite_roundtrip_persists_usage`

17. Matrix store introspection + transition safety
- Added matrix store introspection API:
  - C-ABI: `harmonia_harmonic_matrix_get_store`
  - Tool op: `tool op=matrix-get-store`
- Added graph adapter contract path:
  - `kind=graph` is recognized but returns explicit \"pending implementation\" error.
- Transition safety fix:
  - failed `matrix-set-store kind=graph` no longer mutates active store config.
- Added deterministic test:
  - `graph_store_contract_returns_error` (asserts failed graph transition keeps prior store kind).

18. Additional backend modularization (openrouter-backend)
- Split monolithic crate into:
  - `lib/backends/openrouter-backend/src/client.rs`
  - `lib/backends/openrouter-backend/src/state.rs`
  - `lib/backends/openrouter-backend/src/ffi.rs`
  - `lib/backends/openrouter-backend/src/lib.rs` (module wiring only)
- Verified online completion path after split (`OPENROUTER_MOD_OK`).

19. De-hardcoding pass: runtime config DB and policy routing cleanup
- Added new core crate: `lib/core/config-store` (SQLite-backed runtime config KV).
- New C-ABI surface:
  - `harmonia_config_store_init`
  - `harmonia_config_store_set`
  - `harmonia_config_store_get`
  - `harmonia_config_store_list`
  - `harmonia_config_store_last_error`
  - `harmonia_config_store_free_string`
- Added Lisp backend bridge: `src/backends/config-store.lisp`.
- Boot now initializes config store before policy load:
  - `init-config-store-backend` called in `harmonia:start`.
- Added agent tool ops:
  - `tool op=config-set key=<k> value=<v>`
  - `tool op=config-get key=<k>`
  - `tool op=config-list`
- OpenRouter backend hardcoded model defaults removed:
  - backend now resolves default model via config keys (`openrouter.default_model`, `model.default`).
  - fallback chain now resolved from config key `openrouter.fallback_models` or env override.
- Lisp model policy hardcoded fallback model list removed:
  - fallback/plan defaults are now derived from loaded profiles and seeded into config DB.
- Parallel policy state path and width moved toward runtime config:
  - `parallel.policy.path`, `parallel.subagent_count`.
- Harmonic matrix route defaults and topology path moved toward runtime config:
  - `matrix.route.signal_default`, `matrix.route.noise_default`, `matrix.topology.path`.

20. Validation of new runtime config path
- `cargo test --workspace`: pass.
- Live SBCL config-store smoke:
  - set/get/list of runtime config keys works.
  - matrix route defaults persisted/retrieved through config path.
- Live OpenRouter no-hardcoded-model smoke:
  - `backend-complete` with `model=nil` succeeds via `openrouter.default_model` in config DB (`CFG_MODEL_OK`).

21. Path de-hardcoding sweep (state-root routing)
- Replaced fixed `/tmp/harmonia*` runtime defaults in core Rust modules with:
  - explicit env override (module-specific key), then
  - `HARMONIA_STATE_ROOT`, then
  - `std::env::temp_dir()/harmonia`.
- Updated modules:
  - vault store path + legacy migration path
  - fs sandbox root
  - recovery log path
  - push-sns log path
  - phoenix trauma log
  - parallel-agents metrics log path
  - harmonic-matrix sqlite db default path
- Removed hardcoded graph URI fallback (`HARMONIA_MATRIX_GRAPH_URI` now required when graph backend is selected).
- Removed hardcoded ElevenLabs default voice id in orchestration path:
  - runtime resolves from config/env key (`elevenlabs.default_voice`) or explicit prompt value.

22. Policy boundary correction (Lisp data remains Lisp data)
- Kept mutable Lisp policy/state (`model-policy`, `harmony-policy`, matrix topology) file-first in `.sexp`.
- Removed DB-derived model default/fallback behavior from model-policy logic.
- OpenRouter backend no longer depends on config-store for model defaults:
  - direct backend default model comes from env/runtime call arguments only.

## Next Hardening Steps

1. Add matrix policy persistence (serialize/load topology + dynamic weights across restarts).
2. Add matrix-informed adaptive routing (use observed success/cost/latency to adjust edge weights).
3. Add deterministic integration tests for matrix route-deny + tool unplug behavior.
4. Add Exa/Brave API keys to runtime env (or set via vault write API) to activate live search verification scoring.
