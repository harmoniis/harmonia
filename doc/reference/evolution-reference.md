# Evolution Reference

## Evolution Model

Harmonia evolution is the combination of:

1. `Genomic constraints` (identity, constitution, non-negotiable rules).
2. `Epigenetic mutation` (runtime policies, routes, swarm behavior, rewrite outputs).
3. `Versioned memory` (snapshots and changelog/score trajectories).

Primary conceptual source: `../genesis/concepts.md`.

## Evolution Modes

| Mode | Meaning | Runtime Surface |
|---|---|---|
| `:source-rewrite` | patch/code self-modification path (Ouroboros) | `src/ports/evolution.lisp` |
| `:artifact-rollout` | binary rollout signaling path under Phoenix supervision | `src/ports/evolution.lisp` |

## Runtime Components Involved

| Component | Responsibility |
|---|---|
| `src/core/harmonic-machine.lisp` | computes rewrite readiness context |
| `src/core/rewrite.lisp` | runtime rewrite trigger bookkeeping |
| `src/core/evolution-versioning.lisp` | snapshot version management |
| `src/core/signalograd.lisp` | adaptive checkpoint/restore orchestration tied to accepted evolution versions |
| `src/ports/evolution.lisp` | mode dispatch + rollback hooks |
| `lib/core/ouroboros` | crash history and patch artifact APIs |
| `lib/core/phoenix` | supervisor-level restart/rollout control |
| `lib/core/recovery` | canonical crash/restart ledger substrate |
| `lib/core/signalograd` | compact adaptive model whose checkpoint artifact travels with accepted evolution |
| `src/core/introspection.lisp` | runtime self-knowledge, self-compilation, hot-reload, error ring |

## Versioned Evolution State

Two parallel documentation tracks are active and both matter:

1. `doc/evolution/` and `doc/genesis/`
- developer-facing markdown docs

2. `src/boot/evolution/*`
- runtime-adjacent snapshots loaded by Lisp boot (`version.sexp`, `latest/`, `versions/vN/`)
- includes `signalograd.sexp` when adaptive state is checkpointed alongside an accepted evolution snapshot

These should be kept semantically aligned.

## Safety And Validation Gates

Evolution decisions must remain bounded by:

1. constitutional constraints in genesis docs,
2. matrix route constraints,
3. rewrite validation rules from swarm/self-rewrite policy,
4. recovery and rollback path availability,
5. production readiness checks.
6. evolution portability gates (git push status, distributed propagation).

Primary sources:

- `../evolution/rewrite-roadmap.md`
- `../evolution/scorecard.md`
- `../../config/harmony-policy.sexp`

## Evolution Portability

- `harmonia uninstall evolution-export [-o path.tar.gz]` — portable archive.
- `harmonia uninstall evolution-import <archive> [--merge]` — restore/merge.
- Safety gates: checks git push + distributed propagation before allowing uninstall.

## Evolution Topic Coverage Map

| Topic | Source |
|---|---|
| versioning workflow | `../evolution/changelog.md` |
| current runtime state | `../evolution/current-state.md` |
| harmonic matrix topology | `../../config/matrix-topology.sexp` |
| swarm/model policy | `../../config/swarm.sexp`, `../../config/model-policy.sexp` |
| rewrite roadmap | `../evolution/rewrite-roadmap.md` |
| scoring and quality gates | `../evolution/scorecard.md` |
| genomic/epigenetic framing | `../genesis/concepts.md` |
| harmony policy | `../../config/harmony-policy.sexp` |

## Operational Rule

Any new evolution concept must be reflected in this file and in `migration-map.md`.
