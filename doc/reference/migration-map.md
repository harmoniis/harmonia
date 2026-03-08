# Source Coverage Map (Genesis + Evolution)

This file proves coverage from canonical source docs into this reference set.

## Genesis Source Mapping (`src/boot/genesis`)

| Source Doc | Main Concepts | Reference Targets |
|---|---|---|
| `../../src/boot/genesis/README.md` | corpus navigation, reading order, scope | `README.md`, `system-map.md`, this file |
| `../../src/boot/genesis/concepts.md` | harmonic theory, lambdoma, attractors, complexity | `concepts-glossary.md`, `system-map.md` |
| `../../src/boot/genesis/constitution.md` | engineering constitution, code harmony rules | `system-map.md`, `operations-runbook.md` |
| `../../src/boot/genesis/runtime-architecture.md` | full architecture, FFI surfaces, deployment safety | `system-map.md`, `src-runtime-reference.md`, `lib-crate-reference.md`, `policy-and-state-reference.md` |
| `../../src/boot/genesis/ports-and-ffi.md` | port model, C-ABI surface, CFFI loading | `src-runtime-reference.md`, `lib-crate-reference.md`, `system-map.md` |
| `../../src/boot/genesis/gateway-frontends.md` | baseband model, signal semantics, frontend contract | `system-map.md`, `src-runtime-reference.md`, `lib-crate-reference.md` |

## Evolution Source Mapping (`src/boot/evolution/latest`)

| Source Doc | Main Concepts | Reference Targets |
|---|---|---|
| `../../src/boot/evolution/latest/changelog.md` | historical evolution events | `evolution-reference.md`, `operations-runbook.md` |
| `../../src/boot/evolution/latest/current-state.md` | runtime readiness, security kernel, matrix state | `evolution-reference.md`, `policy-and-state-reference.md` |
| `../../src/boot/evolution/latest/rewrite-roadmap.md` | stability, token efficiency, memory, evolution safety | `evolution-reference.md`, `operations-runbook.md` |
| `../../src/boot/evolution/latest/scorecard.md` | score trajectory, acceptance gates | `evolution-reference.md`, `concepts-glossary.md` |

## Coverage Rule

If a new canonical source file appears in:

- `src/boot/genesis/`, or
- `src/boot/evolution/latest/`,

it must be added here in the same change.
