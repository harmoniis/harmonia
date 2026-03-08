# Evolution Reference

## Evolution Model

Harmonia evolution is the combination of:

1. `Genomic constraints` (identity, constitution, non-negotiable rules).
2. `Epigenetic mutation` (runtime policies, routes, swarm behavior, rewrite outputs).
3. `Versioned memory` (snapshots and changelog/score trajectories).

Primary conceptual source: `../../../doc/agent/evolution/latest/GENOMIC_MODEL.md`.

## Evolution Modes

| Mode | Meaning | Runtime Surface |
|---|---|---|
| `:source-rewrite` | patch/code self-modification path (Ouroboros) | `src/ports/evolution.lisp`, genesis `SELF_REWRITE.md` |
| `:artifact-rollout` | binary rollout signaling path under Phoenix supervision | `src/ports/evolution.lisp`, genesis `ARCHITECTURE.md` |

## Runtime Components Involved

| Component | Responsibility |
|---|---|
| `src/core/harmonic-machine.lisp` | computes rewrite readiness context |
| `src/core/rewrite.lisp` | runtime rewrite trigger bookkeeping |
| `src/core/evolution-versioning.lisp` | snapshot version management |
| `src/ports/evolution.lisp` | mode dispatch + rollback hooks |
| `lib/core/ouroboros` | crash history and patch artifact APIs |
| `lib/core/phoenix` | supervisor-level restart/rollout control |
| `lib/core/recovery` | canonical crash/restart ledger substrate |

## Versioned Evolution State

Two parallel documentation tracks are active and both matter:

1. `../../../doc/agent/evolution/*`
- long-form evolving architecture and historical changelog/score files

2. `src/boot/evolution/*`
- runtime-adjacent snapshots loaded by Lisp boot (`version.sexp`, `latest/`, `versions/vN/`)

These should be kept semantically aligned.

## Safety And Validation Gates

Evolution decisions must remain bounded by:

1. constitutional constraints in genesis docs,
2. matrix route constraints,
3. rewrite validation rules from swarm/self-rewrite policy,
4. recovery and rollback path availability,
5. production readiness checks.

Primary sources:

- `../../../doc/agent/genesis/SELF_REWRITE.md`
- `../../../doc/agent/evolution/latest/RECOVERY.md`
- `../../../doc/agent/evolution/latest/PROD_READINESS.md`
- `../../../doc/agent/evolution/latest/TOKEN_HARMONY.md`

## Evolution Topic Coverage Map

| Topic | Source |
|---|---|
| versioning workflow | `../../../doc/agent/evolution/EVOLUTION.md` |
| current tools/runtime state | `../../../doc/agent/evolution/latest/TOOLS.md` |
| harmonic matrix policy/runtime ops | `../../../doc/agent/evolution/latest/HARMONIC_MATRIX.md` |
| swarm/model policy | `../../../doc/agent/evolution/latest/SWARM_POLICY.md` |
| memory schema | `../../../doc/agent/evolution/latest/MEMORY_SCHEMA.md` |
| recovery role split | `../../../doc/agent/evolution/latest/RECOVERY.md` |
| genomic/epigenetic framing | `../../../doc/agent/evolution/latest/GENOMIC_MODEL.md` |
| token harmony extensions | `../../../doc/agent/evolution/latest/TOKEN_HARMONY.md` |
| production gate evidence | `../../../doc/agent/evolution/latest/PROD_READINESS.md` |

## Operational Rule

Any new evolution concept added under `doc/agent/evolution/latest/` must be reflected in this file and in `migration-map.md`.
