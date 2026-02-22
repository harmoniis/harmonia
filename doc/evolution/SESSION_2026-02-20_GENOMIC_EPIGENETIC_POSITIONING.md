# Session 2026-02-20: Genomic/Epigenetic Positioning Cleanup

## Goal

Clarify how Harmonia differs from other self-improving agents, and remove confusing "DNA + Body" wording.

## Changes

- Frontend (`harmoniis/frontend/src/routes/harmonia/+page.svelte`)
  - Reframed distributed evolution as **Genomic + Epigenetic**.
  - Added explicit differentiation statement:
    - hot patchable,
    - source/policy evolution + runtime adaptation,
    - rollback on patch failure,
    - transparent scoring.

- Frontend metadata
  - `harmoniis/frontend/src/routes/harmonia/+page.server.ts`
  - `harmoniis/frontend/src/routes/+layout.svelte`
  - Updated descriptions/keywords to include genomic, epigenetic, hot patching.

- Code-level DNA model
  - `src/dna/dna.lisp`
  - Added `:evolution-architecture` in immutable DNA:
    - `:genomic`
    - `:epigenetic`
    - `:instrument-layer`
    - `:hot-patch-loop`
  - Included in `dna-soul-sexp` so this identity persists in soul memory.

- Genesis docs
  - `doc/genesis/SBCL.md`
  - `doc/genesis/CONTEXT.md`
  - `doc/genesis/INDEX.md`
  - `doc/genesis/CICD.md`
  - `doc/genesis/ARCHITECTURE.md`
  - Replaced "DNA + Body" framing with genomic/epigenetic terminology and clarified that Rust tools are the instrument/runtime layer.

