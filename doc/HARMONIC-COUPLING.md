# Harmonia — Harmonic Coupling Map & Sensitive Invariants

**What this is.** Harmonia is not a pile of subsystems; it is one coupled dynamical system. A change in
one component propagates through others, and several couplings have a *sensitive invariant* — a property
that, if an upstream change violates it, silently degrades a downstream component. This document is the
single source of truth for those relations. Every fix to a coupled component must preserve the invariants
below, and `scripts/dev/invariant-probe.lisp` asserts them end-to-end.

Convention: **U → D via M** means "U feeds D through mechanism M"; **INVARIANT** is what must hold;
**BREAKS** is the failure mode if it doesn't.

---

## Chain 1 — Meditation → field graph → snapshot → warm-start → spectral recall

**Flow.**
- `memory-meditate` → `%strengthen-edges` (`src/memory/store/concept-map.lisp`) mutate
  `*memory-concept-edges*` weights (`(+ weight boost)`, boost=2 success / 1 fail) and bridge fresh pairs at
  the co-activation threshold.
- `memory-map-sexp :edge-limit 300` (`src/ports/chronicle.lisp:141`) sorts edges **by weight descending**
  and truncates → `chronicle-record-graph-snapshot` writes it to `graph_snapshots`.
- On boot, `%merge-graph-snapshot-into-field` (concept-map.lisp) restores those edges → `memory-field-load-graph`
  pushes the graph to Rust → `lib/core/memory-field/src/spectral.rs` computes the graph-Laplacian eigenpairs;
  `recall.rs` ranks recall by heat-kernel / eigenmode activation.
- The `:observe` harmonic phase also denoises via `memory-map-sexp :edge-limit 160` (`harmonic-machine.lisp:251`).

**INVARIANT (two-sided).** The edge-weight distribution must be **bounded in magnitude AND retain variance/
ordering**.
- **BREAKS (runaway):** weights are currently unbounded — 50 co-activations of one pair grew it 1→101
  (probed, cognition-probe C8). A few hot edges then dominate the by-weight sort and **crowd structural edges
  out of the 300-cut**, so the snapshot (and thus the 1B warm-start) loses the graph skeleton → the restored
  Laplacian is missing its connective edges → degenerate spectrum (the P1 "eigenvalues≈0" regression returns).
- **BREAKS (flattening):** if a fix instead clamps all weights to one value, node degrees become uniform → the
  Laplacian is near-singular → small eigenvalues → heat-kernel `exp(-tλ)` suppresses all modes → recall
  activations go uniform (no structure). **So a magnitude cap that destroys variance is just as wrong as
  runaway.** The correct fix is an **asymptotic, order-preserving** update toward `W_max` (`w + boost*(1-w/W_max)`)
  plus gentle decay (forgetting), which bounds magnitude while keeping the relative ordering that the spectrum
  needs.

**Guarded by:** W2 (bounded meditation) + invariant-probe "sustained diverse meditation → max ≤ W_max AND
variance>0 AND ≥1 eigenvalue>ε / coherence not collapsed."

---

## Chain 2 — Domain assignment → palace tunnels + domain-aware recall

**Flow.**
- `%concept-domain` (`concept-map.lisp`) tags each concept with one of the **fixed 7** domains. The set is
  **mirrored by the Rust `Domain` enum** (`lib/core/mempalace/src/graph.rs`): music, math, engineering,
  cognitive, life, generic, system.
- Domain is consumed by palace wing/tunnel construction (`%palace-build-entry-graph`, `src/ports/mempalace.lisp`):
  same-domain edges weight 0.55, cross-domain "bridges" 0.75, and a **tunnel** node is created when an entry
  spans >1 domain. It also feeds field basin affinity in `recall.rs`.

**INVARIANT (parity + accuracy).** (a) The Lisp domain set must stay **equal to** the Rust enum — never add a
domain Lisp-only. (b) Domain assignment must reflect the concept's actual subject.
- **BREAKS (parity):** a Lisp-only domain string the Rust enum doesn't know collapses to Generic in the basin
  classifier — silent mis-classification.
- **BREAKS (accuracy):** the current static ~5-word-per-domain list sends **12/12 probed technical terms**
  (rk4/lorenz/attractor/ipc/sexp/eigenvalue/laplacian/spectral/chronicle/signalograd…) to `:generic`
  (cognition-probe A2). So mixed technical memories form **spurious `:generic` bridges** instead of real
  math↔engineering tunnels, and domain-aware recall can't tier them. The memory-field is blind to exactly the
  vocabulary the agent works in.

**Guarded by:** W3 (seed map → neighbour-majority inheritance → :generic, mapping only into the 7) +
invariant-probe "technical content classifies non-generic AND a mixed-domain memory builds a real tunnel."

---

## Chain 3 — Eval → fluency → scoring → selection → signalograd → matrix → evolution

**Flow + loop status.**
- `%record-repl-perf` → `%repl-fluency` (`src/core/repl-loop.lisp`) → `%seed-score-with-bias`
  (`src/core/model-routing.lisp`, multiplies base score by fluency, modulated by `signalograd-routing-weight`)
  → `%selection-chain-tiered`/`choose-model` → `backend-complete` → eval → back to `%record-repl-perf`.
  **CLOSED + proven** (cognition-probe B3: 15 code-errors collapsed the chosen model's fluency to 0.0 and
  flipped `choose-model`).
- `model-policy-record-outcome` → per-model `:success-rate` (up on success, down on failure) → scoring
  (`w-success`). **CLOSED + proven** (cognition-probe C5).
- `signalograd` observe/feedback (on `:stabilize`) → projection deltas → `signalograd-routing-weight`
  (clamped [0.05,0.70]) modulates scoring. **CLOSED + proven** (cognition-probe C7; kernel cycle advances on
  observe).
- `harmonic-matrix-observe-route` (`src/ports/matrix.lisp`) records `orchestrator → tool/route` edges
  (uses/success/latency). **OPEN:** neither the conductor nor `%selection-chain-tiered` consults
  `harmonic-matrix-route-allowed-p`/route success → recorded experience does **not** steer future routing.

**INVARIANT.** Closing the matrix loop must remain a **secondary, gated** signal: it must not double-count with
fluency/`success-rate`/signalograd (which already steer selection) nor override the proven free-default and
eval→selection flip.
- **BREAKS:** a strong matrix term that overlaps the existing success signals could destabilize the validated
  selection (oscillation, premium lock-in, or starving the free common-case).

**Guarded by:** W4 (gate + small bias at the orchestrator route/mode granularity the matrix actually records) +
invariant-probe "poor matrix-history route is deprioritized while eval→selection (B3) and free-default still hold."

---

## Chain 4 — Harmonic-machine FSM is the conductor of self-maintenance (and the missing self-improvement)

**Flow.** `harmonic-state-step` (`src/core/harmonic-machine.lisp`) cycles 9 phases (~5s each, harmonic actor):
`:observe` (push field graph) → `:evaluate-global/local` → `:logistic-balance` → `:lambdoma-project` →
`:attractor-sync` (step basins, `memory-field-step-attractors`) → `:rewrite-plan` → `:security-audit` →
`:stabilize` (record harmonic + graph snapshot, dispatch signalograd observe/feedback, checkpoint, maybe
rewrite routing rules) → repeat.

**The gap.** `memory-meditate` and `memory-field-dream` are invoked **only** by REPL primitives
(`%prim-meditate`/`%prim-dream`) — i.e. only when the LLM chooses to call `(meditate)`/`(dream)`. They are in
**no harmonic phase**. So self-*maintenance* (snapshots, checkpoints, signalograd) runs continuously, but
self-*improvement* (Hebbian strengthening, structural compression) runs only by the model's whim — which a
weak/free model rarely exercises. "Recursive improvement" therefore doesn't actually recur on its own.

**INVARIANT.** Auto-scheduling meditate/dream must be **frequency-bounded** and rely on Chain-1 boundedness —
otherwise it reproduces the signalograd "32,886× idle steps" runaway. Meditation per `:stabilize` is fine
(saturating via W2); dream on a lower sub-cadence (heavier; applies decay).
- **BREAKS:** unbounded cadence × unbounded weights (pre-W2) = compounding runaway that degrades Chain 1.

**Guarded by:** W5 (cadence-scheduled, bounded) + invariant-probe "over K cycles with no manual calls,
meditate/dream fire automatically AND the field stays bounded + coherence non-decreasing."

---

## Chain 3b — Tier → selection (FINDING 2026-06-05: the tier barely gates selection)
Headless validation under `:premium`: `%select-model-by-repl-perf` correctly pooled premium and picked
`grok-4.1-fast`, **but** `choose-model "implement a feature"` returned `qwen…:free` — the fallback/task path
does **not** honor the tier pool. Also `/premium` sent over the TUI socket did **not** change `*routing-tier*`.
And under `:auto`, `%repl-model-score` (0.5·fluency + 0.3·speed + 0.2·cost) makes the **free** model win every
turn (cost=0, most-exercised) — so in practice the agent runs free regardless of intent. The OpenRouter key
DOES reach premium (`backend-complete(…, "anthropic/claude-opus-4.6")` → "42").
**INVARIANT:** the active tier must gate **every** selection path (repl-perf AND choose-model AND task-specific),
and `:auto` must escalate hard/critical tasks to premium — else "auto models from OpenRouter" is just "free."
**Folded into the routing work (W4):** make `choose-model`/task routing respect `%tier-model-pool`, make the
`/premium`/`/auto` command actually set+reload the tier, and let `:auto` escalate on task difficulty.

## Routing tiers (context for auto-mode verification)
`*routing-tier*` defaults to **`:auto`** (`src/core/model-routing.lisp:7`) — NOT free. `:auto` admits the whole
pool and scoring decides; trivial prompts correctly route to the free pool, complex/`:critical-reasoning`/
`:software-dev` prompts (and repeated REPL failure, via `model-escalation-chain`) escalate toward premium.
OpenRouter (`lib/backends/llms/openrouter/`) is the **universal fallback** for any model id whose native
provider key is absent; the key lives in the vault under `openrouter/openrouter-api-key`. There is **no**
`openrouter/auto` meta-model — "auto" is Harmonia's tier. Verification must therefore exercise simple **and**
complex prompts to see the full free→premium span, and report the model each decision actually picked.

---

## Discarded (code-reading errors — trust the running system)
- "`signalograd-routing-weight` is undefined" — **false**; it is defined and works (cognition-probe C7 proved
  routing weights are live and clamped).
- "`security-note-event` is never called" — **false**; it is called from gateway poll on dissonance.
