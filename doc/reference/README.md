# Harmonia Reference Atlas

This folder is the developer-facing reference for Harmonia.

It is not a replacement for canonical long-form docs. It is the structured map that links concepts, runtime modules, policies, and operations back to source-of-truth documentation.

## Canonical Sources

| Source | Scope | Purpose |
|---|---|---|
| `../../../doc/agent/genesis/*.md` | full genesis architecture corpus | authoritative concept and architecture narrative |
| `../../../doc/agent/evolution/latest/*.md` | current evolution state | authoritative current policy/runtime behavior |
| `../../../doc/agent/evolution/EVOLUTION.md` | versioning workflow | authoritative snapshot process |
| `../../src/boot/genesis/*.md` | runtime-adjacent genesis subset | concise bootstrap knowledge used near runtime |
| `../../src/boot/evolution/*` | runtime snapshot state | versioned evolution memory loaded by Lisp boot |

## Reference Documents In This Folder

| File | What It Covers | Primary Sources |
|---|---|---|
| `system-map.md` | end-to-end architecture and flow topology | genesis `CONTEXT.md`, `ARCHITECTURE.md`, `GATEWAY.md`, `SWARM.md` |
| `signalograd-architecture.md` | chaos-computing adaptive kernel, learning rules, actor integration, persistence, audit boundaries | `lib/core/signalograd`, `src/core/signalograd.lisp`, evolution snapshot docs |
| `src-runtime-reference.md` | Lisp runtime modules, boot order, port boundaries | `src/core/*`, `src/orchestrator/*`, `src/ports/*`, genesis `ARCHITECTURE.md` |
| `lib-crate-reference.md` | Rust crate inventory by pillar | `Cargo.toml`, evolution `TOOLS.md`, genesis `ARCHITECTURE.md` |
| `policy-and-state-reference.md` | config/state files, env overrides, persistence boundaries | `config/*.sexp`, core/port policy loaders, evolution `HARMONIC_MATRIX.md` |
| `evolution-reference.md` | rewrite model, versioning model, safety gates | genesis `SELF_REWRITE.md`, evolution `EVOLUTION.md`, `RECOVERY.md`, `GENOMIC_MODEL.md` |
| `operations-runbook.md` | startup checks, verification commands, recovery workflow | scripts, evolution `PROD_READINESS.md`, genesis `GENESIS_DEV_FLOW.md` |
| `security-architecture.md` | comprehensive security architecture reference | security kernel, adaptive shell, transport security, threat model |
| `distributed-evolution-todo.md` | publish/subscribe algorithm backlog for org-wide evolution | evolution ports, swarm policy, s3 storage model |
| `concepts-glossary.md` | normalized vocabulary across docs | genesis and evolution corpora |
| `migration-map.md` | explicit source-to-reference coverage matrix | `doc/agent/genesis/*`, `doc/agent/evolution/latest/*` |
| `source-section-coverage.md` | generated heading-level coverage index | generated from canonical docs + `migration-map.md` |

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

## Usage Order

1. Start with `system-map.md`.
2. Read `signalograd-architecture.md` when changing adaptive learning behavior.
3. Read `migration-map.md` to find the exact source doc for a concept.
4. Use `src-runtime-reference.md` and `lib-crate-reference.md` for code navigation.
5. Use `policy-and-state-reference.md` and `operations-runbook.md` for runtime changes.
6. Use `evolution-reference.md` when changing self-modification behavior.

## Maintenance Rules

1. If a concept exists in `doc/agent/genesis` or `doc/agent/evolution/latest`, it must appear in `migration-map.md`.
2. Never reference non-existent paths.
3. Keep source links explicit; do not paraphrase away critical constraints.
4. Update this reference whenever new evolution topic files are added.
5. Regenerate `source-section-coverage.md` with `scripts/generate-doc-section-coverage.sh` after canonical doc changes.
