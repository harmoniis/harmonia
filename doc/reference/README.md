# Harmonia Reference Atlas

This folder is the developer-facing reference for Harmonia.

It is not a replacement for canonical long-form docs. It is the structured map that links concepts, runtime modules, policies, and operations back to source-of-truth documentation.

## Canonical Sources

| Source | Scope | Purpose |
|---|---|---|
| `../../src/boot/genesis/*.sexp` | genesis architecture (agent-facing) | authoritative concept and architecture narrative |
| `../../src/boot/evolution/latest/*.sexp` | current evolution state (agent-facing) | authoritative current policy/runtime behavior |
| `../genesis/*.md` | genesis docs (developer-facing) | markdown mirrors of boot genesis |
| `../evolution/*.md` | evolution docs (developer-facing) | markdown mirrors of boot evolution |
| `../../config/*.sexp` | runtime configuration | policies, topology, prompts, model routing |

## Reference Documents In This Folder

| File | What It Covers | Primary Sources |
|---|---|---|
| `system-map.md` | end-to-end architecture and flow topology | `../genesis/runtime-architecture.md`, `../genesis/gateway-frontends.md`, `../genesis/concepts.md` |
| `signalograd-architecture.md` | chaos-computing adaptive kernel, learning rules, actor integration, persistence, audit boundaries | `lib/core/signalograd`, `src/core/signalograd.lisp` |
| `src-runtime-reference.md` | Lisp runtime modules, boot order, port boundaries | `src/core/*`, `src/orchestrator/*`, `src/ports/*` |
| `lib-crate-reference.md` | Rust crate inventory by pillar | `Cargo.toml`, `config/tools.sexp` |
| `policy-and-state-reference.md` | config/state files, env overrides, persistence boundaries | `config/*.sexp`, core/port policy loaders |
| `evolution-reference.md` | rewrite model, versioning model, safety gates | `../evolution/*.md`, `../../src/boot/evolution/latest/*.sexp` |
| `operations-runbook.md` | startup checks, verification commands, recovery workflow | `../evolution/scorecard.md` |
| `security-architecture.md` | comprehensive security architecture reference | security kernel, adaptive shell, transport security, threat model |
| `distributed-evolution-todo.md` | publish/subscribe algorithm backlog for org-wide evolution | evolution ports, swarm policy, s3 storage model |
| `memory-as-a-field.md` | memory field architecture: field propagation, attractor basins, spectral recall, topological pruning, actor integration | `src/memory/store/concept-map.lisp`, `lib/core/chronicle/src/tables/graph.rs`, `lib/core/signalograd` |
| `memory-field-theory.md` | theoretical foundations: attractor dynamics, hysteresis, spectral graph theory, Kolmogorov pruning, context collapse | `memory-as-a-field.md`, `signalograd-architecture.md` |
| `memory-field-crate.md` | memory-field Rust crate: Laplacian solver, spectral cache, activation engine, topology analysis, IPC port | `lib/core/memory-field/`, `lib/core/chronicle/`, `lib/core/signalograd/` |
| `memory-field-observations.md` | test results (71 tests), basin state insight, cold-start analysis, encoding path, multi-modal proposals, improvement roadmap | `lib/core/memory-field/tests/`, `memory-as-a-field.md` |
| `concepts-glossary.md` | normalized vocabulary across docs | genesis and evolution corpora |
| `migration-map.md` | source-to-reference coverage matrix | `../genesis/*`, `../evolution/*` |
| `source-section-coverage.md` | heading-level coverage index | generated from canonical docs |

## Coverage Domains Guaranteed

This reference set explicitly covers:

1. System architecture and component boundaries.
2. Harmonic theory and constitutional constraints.
3. Ports/baseband/router/swarm/matrix/evolution topology.
4. Rust crate layout (core/backends/tools/frontends).
5. Memory model and runtime policy surfaces.
6. Self-rewrite, recovery, and versioned evolution snapshots.
7. UI/UX and A2UI concept surfaces (not ignored).
8. CI/CD, genesis development flow, and operational readiness.
9. Memory field dynamics: attractor-based recall, spectral decomposition, topological pruning.

## Usage Order

1. Start with `system-map.md`.
2. Read `signalograd-architecture.md` when changing adaptive learning behavior.
3. Read `migration-map.md` to find the exact source doc for a concept.
4. Use `src-runtime-reference.md` and `lib-crate-reference.md` for code navigation.
5. Use `policy-and-state-reference.md` and `operations-runbook.md` for runtime changes.
6. Use `evolution-reference.md` when changing self-modification behavior.
7. Read `memory-as-a-field.md` when changing memory recall, concept graph dynamics, or attractor-based routing.

## Maintenance Rules

1. If a concept exists in `doc/genesis` or `doc/evolution`, it must appear in `migration-map.md`.
2. Never reference non-existent paths.
3. Keep source links explicit; do not paraphrase away critical constraints.
4. Update this reference whenever new evolution topic files are added.
5. Regenerate `source-section-coverage.md` with `scripts/generate-doc-section-coverage.sh` after canonical doc changes.
