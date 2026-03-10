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
| `signalograd.sexp` | `lib/core/signalograd` | persisted local Signalograd kernel state for adaptive reflection and online attractor learning |
| `recovery.log` | recovery/ouroboros/phoenix flows | crash/restart ledger |
| `vault.db` | vault crate | encrypted secret store |
| `chronicle.db` | chronicle crate | SQL-queryable knowledge base (harmonic snapshots, delegation, graph, `signalograd_events`) |

## Evolution Snapshot State (`src/boot/evolution/`)

| Path | Role |
|---|---|
| `src/boot/evolution/latest/*` | current mutable evolution snapshot, including `signalograd.sexp` checkpoints |
| `src/boot/evolution/versions/vN/*` | immutable evolution history snapshots, including version-matched `signalograd.sexp` |
| `src/boot/evolution/version.sexp` | current version integer used at boot |

## Key Environment Overrides

All config keys are resolved through config-store with the fallback chain: cache → DB → registry-derived env var → default. Env var names are derived from `(scope, key)` pairs by the config-store registry (see `lib/core/config-store/src/registry.rs`).

| Variable | Scope / Key | Effect |
|---|---|---|
| `HARMONIA_ENV` | `global / env` | runtime environment mode |
| `HARMONIA_ALLOW_PROD_GENESIS` | `phoenix-core / allow-prod-genesis` | explicit production bootstrap override |
| `HARMONIA_STATE_ROOT` | `global / state-root` | root for mutable runtime state files |
| `HARMONIA_MODEL_POLICY_PATH` | `model-policy / path` | override model-policy state path |
| `HARMONIA_HARMONY_POLICY_PATH` | `harmony-policy / path` | override harmony-policy state path |
| `HARMONIA_MATRIX_TOPOLOGY_PATH` | `harmonic-matrix / topology-path` | override matrix-topology state path |
| `HARMONIA_PARALLEL_POLICY_PATH` | `parallel-agents-core / policy-path` | override swarm state path |
| `HARMONIA_ROUTE_SIGNAL_DEFAULT` | `harmonic-matrix / route-signal-default` | default matrix route signal |
| `HARMONIA_ROUTE_NOISE_DEFAULT` | `harmonic-matrix / route-noise-default` | default matrix route noise |
| `HARMONIA_MODEL_PLANNER` | `model-policy / planner` | enable/disable planner model selection |
| `HARMONIA_MODEL_PLANNER_MODEL` | `model-policy / planner-model` | explicit planner model id |
| `HARMONIA_LIB_DIR` | `global / lib-dir` | override platform library directory |
| `HARMONIA_SOURCE_DIR` | `global / source-dir` | override source directory (share dir) |
| `HARMONIA_SIGNALOGRAD_STATE_PATH` | `signalograd-core / state-path` | persisted kernel working-state path |
| `HARMONIA_LOG_LEVEL` | `global / log-level` | log verbosity (debug/info/warn/error) |
| `HARMONIA_CHRONICLE_DB` | `chronicle / db` | override chronicle database path |

## Signalograd Config Keys

| Scope | Key | Purpose |
|---|---|---|
| `signalograd-core` | `state-path` | persisted kernel working-state path |

## Config-Store Seed Keys (`scope = model-policy`)

These keys are populated by setup and consumed by `src/core/model-policy.lisp`:

| Key | Purpose |
|---|---|
| `provider` | active provider id used for provider-scoped seed lookup |
| `seed-models` | active provider seed list (CSV, user-editable) |
| `seed-models-<provider>` | provider-specific default/override seed list (CSV) |

Operational note: `harmonia setup --seeds` updates these keys without re-running full setup.

## MQTT Broker And Remote Config Keys

These keys are populated by setup and refreshed by the embedded broker process:

| Scope | Key | Purpose |
|---|---|---|
| `mqtt-broker` | `mode` | `embedded` or `external` |
| `mqtt-broker` | `bind` | local listener address for the embedded broker |
| `mqtt-broker` | `tls` | whether the broker requires TLS |
| `mqtt-broker` | `ca-cert` | CA cert path used for mutual TLS client verification |
| `mqtt-broker` | `server-cert` | broker certificate chain path used by `rmqtt` for server identity and client-cert trust roots |
| `mqtt-broker` | `server-key` | broker private key path |
| `mqtt-broker` | `remote-config-url` | signed remote API endpoint for agent config reads (`/api/agent`) |
| `mqtt-broker` | `remote-config-identity-label` | vault-derived wallet label used for signing |
| `mqtt-broker` | `remote-config-refresh-seconds` | refresh cadence for remote config sync |
| `mqtt-frontend` | `broker` | MQTT broker host:port the frontend connects to |
| `mqtt-frontend` | `trusted-client-fingerprints-json` | cached trusted MQTT client identity list |
| `mqtt-frontend` | `trusted-device-registry-json` | cached push/device registry fetched from remote config |
| `mqtt-frontend` | `push-webhook-url` | backend push webhook endpoint |
| `mqtt-frontend` | `push-webhook-token` | optional push webhook bearer token |

Operational note: the embedded broker uses `rmqtt` with `cert_cn_as_username`, so the wallet-derived certificate CN must be the normalized client fingerprint. Offline device messages are persisted in `mqtt-offline-queue.db` under `HARMONIA_STATE_ROOT`.

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
