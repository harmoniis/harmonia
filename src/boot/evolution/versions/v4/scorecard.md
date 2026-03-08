# Scorecard

This scorecard tracks the quality dimensions that matter for Harmonia rewrites.

## Primary Dimensions

1. Completion reliability
2. Correctness and error containment
3. Token efficiency
4. Orchestration efficiency (tool-chain quality)
5. Harmonic signal/noise
6. Rewrite safety (rollback and recovery path health)

## Observed Runtime Metrics

- Per-response harmonic score (`src/harmony/scorer.lisp`)
- Route telemetry from harmonic matrix (`observe_route`, timeseries, reports)
- Swarm experience scores (`swarm_model_scores.sexp`)
- Recovery signals (ouroboros/recovery logs)

## Scoring Intent

A rewrite is considered healthy when it improves at least one primary dimension without violating:

- DNA invariants,
- matrix constraints,
- vault boundaries,
- or rollback safety.

## Minimal Acceptance Gate

A candidate rewrite should pass:

- no startup regression,
- no route permission regression for required paths,
- no increase in unresolved runtime errors,
- stable or improved harmonic signal/noise profile.
