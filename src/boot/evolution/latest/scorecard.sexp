(:title "Scorecard"
 :purpose "Tracks the quality dimensions that matter for Harmonia rewrites."

 :primary-dimensions
  ("Completion reliability"
   "Correctness and error containment"
   "Token efficiency"
   "Orchestration efficiency (tool-chain quality)"
   "Harmonic signal/noise"
   "Rewrite safety (rollback and recovery path health)"
   "Security posture (injection resistance, taint integrity, policy gate coverage)")

 :observed-runtime-metrics
  ("Per-response harmonic score (src/harmony/scorer.lisp)"
   "Route telemetry from harmonic matrix (observe_route, timeseries, reports)"
   "Swarm experience scores (swarm_model_scores.sexp)"
   "Recovery signals (ouroboros/recovery logs)"
   "Security posture state (*security-posture*, injection counts, policy gate denials)"
   "Per-frontend dissonance scores and anomaly rates"
   "Supervision health: *tick-error-count*, *consecutive-tick-errors*"
   "Library health: introspect-libs crash counts and status per loaded module"
   "Error ring: introspect-recent-errors for last N errors with context"
   "Full diagnostic: introspect-runtime snapshot"
   "Chronicle harmonic snapshots: full vitruvian + chaos + lorenz + lambdoma per cycle"
   "Chronicle harmony trajectory: 5-minute downsampled signal evolution (never pruned)"
   "Chronicle delegation log: model, cost, latency, tokens, success per LLM call"
   "Chronicle memory events: crystallisation/compression with sizes and ratios"
   "Chronicle concept graph: decomposed nodes/edges with recursive CTE traversal"
   "Chronicle lifecycle: phoenix supervisor + ouroboros self-repair events"
   "Chronicle pressure: DB size and GC tier via chronicle-gc-status")

 :scoring-intent "A rewrite is considered healthy when it improves at least one primary dimension without violating DNA invariants, matrix constraints, vault boundaries, or rollback safety."

 :minimal-acceptance-gate
  ("no startup regression"
   "no route permission regression for required paths"
   "no increase in unresolved runtime errors"
   "stable or improved harmonic signal/noise profile"
   "no security regression (policy gate coverage, taint propagation integrity)"
   "no weakening of invariant guards or privileged edge thresholds"
   "no supervision regression (tick actions must remain individually wrapped)"
   "no library crash count increase (steady-state should be zero crashes)"
   "no chronicle recording regression"
   "chronicle DB size within pressure tier expectations (soft < 50MB normal operation)"
   "command dispatch coverage (all /commands intercepted by gateway, security labels enforced)"))
