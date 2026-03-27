# Constitution

## Identity

The Harmonia architecture is a self-improving SBCL Common Lisp + Rust orchestration system. Agent identity is configurable per installation (`config-store: agent/name`).

Source of identity: `src/dna/dna.lisp`.

Invariants:

- Creator: Harmoniq Punk (PGP: `88E016462EFF9672`). Verified by PGP signature.
- DNA validity enforced at startup (`dna-valid-p`).
- Production genesis blocked unless `HARMONIA_ALLOW_PROD_GENESIS=1`.

## Foundation

The system is driven by a mathematical, physical, biological, and philosophical foundation:

- **Vitruvian stoichiometry**: strength × utility × beauty converge (lambdoma ratio ≥ 0.72).
- **Discover harmonies**: gravitate to basin minima. Curiosity discovers, does not impose.
- **Energy is in the fields**: memory is a potential field (L=D-A). Recall is wave propagation.
- **Reduce Kolmogorov complexity**: compression is intelligence. Program growth without new function is degradation.
- **Path of minimum action**: Laplacian field solve finds shortest paths.
- **Functional, not imperative**: code is data, data is code. Generalize instead of adding cases.
- **Lambdoma**: small numbers carry the real harmonic information. Infinity meets nothingness.
- **一期一会**: each moment deserves to live in the present.

## Vitruvian Triad

Computed every harmonic cycle (`src/core/harmonic-machine.lisp`):

- `strength`: resilient under failure, coherent under pressure.
- `utility`: simple things simple, complex things possible.
- `beauty`: consonant structure across all scales.

Signal = 0.34×strength + 0.33×utility + 0.33×beauty. Rewrite readiness requires convergence.

## Foundational Constraints

1. Honor the mathematical foundation — all evolution must preserve harmonic coherence.
2. Keep orchestration composable and auditable.
3. Policy is runtime-loadable (`.sexp`), not hardcoded.
4. Route sensitive operations through vault and matrix boundaries.
5. Evolution must be rollback-capable.
6. Security kernel for all external signals: typed dispatch, policy gate, taint propagation.
7. Never execute `read-from-string` with `*read-eval*` true on external data.
8. Privileged operations require deterministic policy gate — harmonic scoring alone is insufficient.
