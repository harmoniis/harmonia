# Scorecard

This scorecard tracks the quality dimensions that matter for Harmonia rewrites.

## Primary Dimensions

1. Completion reliability
2. Correctness and error containment
3. Token efficiency
4. Orchestration efficiency (tool-chain quality)
5. Harmonic signal/noise
6. Rewrite safety (rollback and recovery path health)
7. Security posture (injection resistance, taint integrity, policy gate coverage)

## Observed Runtime Metrics

- Per-response harmonic score (`src/harmony/scorer.lisp`)
- Route telemetry from harmonic matrix (`observe_route`, timeseries, reports)
- Swarm experience scores (`swarm_model_scores.sexp`)
- Recovery signals (ouroboros/recovery logs)
- Security posture state (`*security-posture*`, injection counts, policy gate denials)
- Per-frontend dissonance scores and anomaly rates
- Supervision health: `*tick-error-count*`, `*consecutive-tick-errors*` (error storm detection)
- Library health: `introspect-libs` crash counts and status per loaded cdylib
- Error ring: `introspect-recent-errors` for last N errors with context
- Full diagnostic: `introspect-runtime` snapshot (platform, paths, libs, errors, frontends)
- Chronicle harmonic snapshots: full vitruvian + chaos + lorenz + lambdoma per cycle (`chronicle-query`)
- Chronicle harmony trajectory: 5-minute downsampled signal evolution (never pruned)
- Chronicle delegation log: model, cost, latency, tokens, success per LLM call
- Chronicle memory events: crystallisation/compression with sizes and ratios
- Chronicle concept graph: decomposed nodes/edges with recursive CTE traversal
- Chronicle lifecycle: phoenix supervisor + ouroboros self-repair events
- Chronicle pressure: DB size and GC tier via `chronicle-gc-status`

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
- no security regression (policy gate coverage, taint propagation integrity),
- no weakening of invariant guards or privileged edge thresholds,
- no supervision regression (tick actions must remain individually wrapped),
- no library crash count increase (steady-state should be zero crashes).
- no chronicle recording regression (harmonic snapshots, delegation, memory events must continue recording).
- chronicle DB size within pressure tier expectations (soft < 50MB normal operation).
