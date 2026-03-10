# Policy And State Reference

## Declarative Config Files (`config/`)

| File | Role |
|---|---|
| `config/tools.sexp` | known crate/tool registry map |
| `config/model-policy.sexp` | model profiles, task-routing, provider-scoped seed defaults, CLI preference/timeout/cooloff, summarizer + delegation policy |
| `config/harmony-policy.sexp` | harmonic numeric thresholds/weights |
| `config/matrix-topology.sexp` | allowed nodes/edges/tool toggles |
| `config/swarm.sexp` | swarm width, tmux policy, rewrite gates |
| `config/baseband.sexp` | frontend auto-load, security labels, capabilities declarations (`:capabilities`), and push webhook config |
| `config/tailnet.sexp` | tailnet mesh transport config |
| `config/a2ui-catalog.sexp` | A2UI component catalog — 21 components with field specs, lazily loaded by conductor for capabilities-driven A2UI dispatch |
| `config/structure-allowlist.txt` | structure constraints for safe evolution |

## Runtime-Mutable State (Default Under `HARMONIA_STATE_ROOT`)

| State File | Produced By | Purpose |
|---|---|---|
| `model-policy.sexp` | `src/core/model-policy.lisp` | mutable model policy state |
| `harmony-policy.sexp` | `src/core/harmony-policy.lisp` | mutable harmony policy state |
| `matrix-topology.sexp` | `src/ports/matrix.lisp` | mutable matrix topology |
| `swarm.sexp` | `src/ports/swarm.lisp` | mutable swarm fan-out state |
| `swarm_model_scores.sexp` | `src/core/model-policy.lisp` | per-model success/latency/cost/vitruvian score history used for seed evolution |
| `recovery.log` | recovery/ouroboros/phoenix flows | crash/restart ledger |
| `vault.db` | vault crate | encrypted secret store |
| `chronicle.db` | chronicle crate | SQL-queryable knowledge base (harmonic snapshots, delegation, graph) |

## Evolution Snapshot State (`src/boot/evolution/`)

| Path | Role |
|---|---|
| `src/boot/evolution/latest/*` | current mutable evolution snapshot |
| `src/boot/evolution/versions/vN/*` | immutable evolution history snapshots |
| `src/boot/evolution/version.sexp` | current version integer used at boot |

## Key Environment Overrides

| Variable | Effect |
|---|---|
| `HARMONIA_ENV` | runtime environment mode |
| `HARMONIA_ALLOW_PROD_GENESIS` | explicit production bootstrap override |
| `HARMONIA_STATE_ROOT` | root for mutable runtime state files |
| `HARMONIA_MODEL_POLICY_PATH` | override model-policy state path |
| `HARMONIA_HARMONY_POLICY_PATH` | override harmony-policy state path |
| `HARMONIA_MATRIX_TOPOLOGY_PATH` | override matrix-topology state path |
| `HARMONIA_PARALLEL_POLICY_PATH` | override swarm state path |
| `HARMONIA_ROUTE_SIGNAL_DEFAULT` | default matrix route signal |
| `HARMONIA_ROUTE_NOISE_DEFAULT` | default matrix route noise |
| `HARMONIA_MODEL_PLANNER` | enable/disable planner model selection |
| `HARMONIA_MODEL_PLANNER_MODEL` | explicit planner model id |
| `HARMONIA_LIB_DIR` | override platform library directory |
| `HARMONIA_SOURCE_DIR` | override source directory (share dir) |

## Config-Store Seed Keys (`scope = model-policy`)

These keys are populated by setup and consumed by `src/core/model-policy.lisp`:

| Key | Purpose |
|---|---|
| `provider` | active provider id used for provider-scoped seed lookup |
| `seed-models` | active provider seed list (CSV, user-editable) |
| `seed-models-<provider>` | provider-specific default/override seed list (CSV) |

Operational note: `harmonia setup --seeds` updates these keys without re-running full setup.

## Policy Boundaries

1. Secrets are not config policy; they live behind vault APIs.
2. Matrix routing policy and tool enablement remain explicit and inspectable.
3. Swarm/model/harmony policy are mutable but persisted and auditable.
4. Evolution snapshots are versioned; state mutation is not anonymous.

## Canonical Cross-References

1. Runtime policy architecture: `../../../doc/agent/evolution/latest/HARMONIC_MATRIX.md`
2. Swarm/model policy details: `../../../doc/agent/evolution/latest/SWARM_POLICY.md`
3. Memory schema policy: `../../../doc/agent/evolution/latest/MEMORY_SCHEMA.md`
4. A2UI policy and component semantics: `../../../doc/agent/genesis/A2UI_SPEC.md`
