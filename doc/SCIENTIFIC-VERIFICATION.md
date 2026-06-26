# Scientific Verification — Harmonia ↔ DYSCO

**Status:** in progress (started 2026-06-26). This is the single source of truth for the audit + formal re-verification campaign. Companion: `doc/DYSCO-COMPARISON.md`. Plan of record: the campaign plan (audit → invariant re-verification → DYSCO comparison → D1 symbolic recovery → D2 well-posedness → e2e probes → gap closure → least-action self-rewrite loop).

**Methodology (binding).** Every load-bearing verdict here is confirmed by *reading the code*, not a summary. A first-pass subagent audit mischaracterized signalograd's crux and reported stale roadmap items as live gaps; both are corrected below from source. Numbers that require running a probe are marked **PENDING** and will be filled with real output — nothing is asserted that a probe did not produce.

---

## A. Component audit — REAL / STUB / MOCK

| Component | Files (evidence) | Verdict |
|---|---|---|
| Memory-field attractors (Thomas/Aizawa/Halvorsen) | `lib/core/memory-field/src/attractor.rs` — `step_thomas:70` is true **RK4** + `soft_saturate(·,3.0)`; Thomas `dx=sin y−bx …` at b≈0.208 | **REAL** |
| Graph-Laplacian field solve | `field.rs` — conjugate gradient on `(L+εI)φ=b` = minimize Dirichlet energy (least-action; `scoring.rs:5` unit Dirichlet BCs) | **REAL** |
| Spectral eigensolver / heat kernel | `spectral.rs` — power iteration + deflation; `exp(−tλ)` activation | **REAL** |
| Hysteresis + topological-flux (A-B) | `basin.rs`, `topology.rs` — ferromagnetic switch + non-decaying cycle circulation `Σ ln(w_fwd/w_rev)` | **REAL** |
| Dreaming (prune/merge/crystallize) | `dream.rs` — Landauer-accounted; `concept-map.lisp:96` decay+prune+cap equilibrium | **REAL** |
| **signalograd** (see §C) | `lib/core/signalograd/src/{kernel,model,feedback,weights,lib}.rs` | **REAL** — Lorenz-reservoir CTRNN, *not* autodiff; readout is exactly extractable (§C) |
| Eval / routing / escalation | `model-routing.lisp`, `repl-loop.lisp` — tier pools, fluency scoring, escalation excludes failed model + reaches premium | **REAL** |
| s-expr evaluator | `src/core/sexp-eval.lisp` (+ 30+ tests) — sandboxed interpreter, denied-operator gate | **REAL** |
| Solomonoff/Kolmogorov compression | `src/memory/store/compression.lisp:35` — `solomonoff-prior=exp(−size/40)`, `kolmogorov-ratio`, `occam-pass≤1.1` | **REAL** (see §B/§D) |
| Lambdoma selection + proof | `operations.lisp:516 %lambdoma-select` + `proofs/Lambdoma.v` (all `Qed.`) + runtime contract `lambdoma-recall-coherent` | **REAL**, narrow (bounded+subset; abstracts the resonance sort) |
| Probe harness | `scripts/dev/{deep,cognition,circuit,persistence,finder,conversation}-probe.*`, `scripts/resonance-probe.lisp` (live LLM) | **REAL** |
| `:rewrite-plan` → actual evolution | `harmonic-machine.lisp:325` computes the readiness **gate**; `rewrite.lisp:3` evolution = **binary rollout, not source rewrite** | Gate REAL; self-rewrite is rollout (the loop the campaign builds — §E) |
| `tests/test-closed-loops.lisp` | mocks palace/LLM/IPC | **MOCK — Lisp-logic test, not integration**; not counted as e2e coverage |

No `todo!()`/`unimplemented!()`/`Admitted.` found in the audited core. The field math is textbook and configured from genome/literature constants (Thomas b∈[0.18,0.24], Lorenz σ=10/ρ=28/β=8/3), not magic numbers.

## B. Roadmap-vs-source gap reconciliation — **3 of 4 "gaps" are already closed**

The subagents read `PRODUCTION-ROADMAP.md`; the current source has moved on. Verified:

| Roadmap gap | Source reality | Status |
|---|---|---|
| Meditation weights grow unbounded `(+ weight boost)` | `%strengthen-edges:308`→`%reinforce-weight:76` = `min(*concept-edge-weight-max*=12, w0+boost)`, clamped to genome `:concept-edge-weight (0.0 . 12.0)`. Kept **linear on purpose** to preserve the denoise ≥2 threshold (a log cap would break it). + decay + prune + count-cap (`:96`). | **CLOSED — correct design, do not "improve"** |
| Domain classifier maps own vocab → `:generic` | `%concept-domain-seed:5` seeds `rk4 lorenz attractor eigenvalue laplacian spectral kolmogorov lambdoma … / sexp ipc signalograd chronicle ractor …`; `%refine-generic-domains:36` adds graph-neighbor majority inheritance on the dream cadence | **CLOSED** |
| Harmonic-matrix records but never steers selection | `model-routing.lisp:173-179` — *"Close the harmonic-matrix → routing loop: consult the matrix"*, calls `harmonic-matrix-route-allowed-p` | **CLOSED** |
| Compound multi-fact recall ("language AND database") | `memory-recall:532` unions field+lexical+finder of a *single* query; a task-level `decompose` primitive exists (`repl-primitives.lisp:853`) but is not an automatic recall-query splitter | **POSSIBLY OPEN — settle with live probe (Phase 7)** |

Action: turn the three closed gaps into *passing* probe assertions; reconcile `PRODUCTION-ROADMAP.md` with source; only build a fix for compound recall if the live probe actually misses.

## C. signalograd — what it actually is

A **plastic CTRNN / echo-state reservoir** (`signalograd-architecture.md:5-12,74-78` explicitly disallows SGD/backprop):
- **Basis Ξ:** a telemetry-modulated *canonical* Lorenz attractor (`weights.rs:17` σ=10/ρ=28/β=8/3, edge of chaos; `update_lorenz:29` modulates σ/ρ/β/dt by telemetry) expanded to 32 latent units via golden-ratio phase harmonics (`lorenz_basis:61`).
- **Coefficients Θ, learned on 3 timescales:** delta+Oja readouts — `w += η(target−prediction)·latent` (`kernel.rs:128`), which **is** the gradient of squared error for the *linear* readout (the "Hebbian not gradient" claim was false); a chaos-energy-modulated meta-update on 119 Kolmogorov-compressed weights (`kernel.rs:511`); Hopfield consolidation of rewarded latents (`feedback.rs:61`).
- **Consequence:** the readout is already in DYSCO's `fˆ(x)=Θ·Ξ(x)` form (their eq. 3). Because Ξ is *fixed and named*, Θ is **exactly symbolically extractable as an s-expression — no affine-gauge freedom, no L1 thresholding** (the two things DYSCO's Appendix C blames for its spurious terms). This **grounds** the "extract the equation and rewrite itself" vision. The `-grad` is the Slavic **-град = citadel**, not "gradient."

## D. Epistemic frame — Solomonoff / Kolmogorov / least-action is *constitutional*, not a bolt-on

`constitution.sexp` makes it law and the code implements it:
- `:17 :reduce-kolmogorov-complexity "Compression is intelligence. Solomonoff prior exp(-size/40)."` → implemented in `compression.lisp:35` (nightly non-destructive memory compression into `:skill` entries tagged `:solomonoff :occam`).
- `:18 :path-of-minimum-action "Laplacian field solve finds shortest paths."` → the `(L+εI)φ=b` CG solve *is* graph-Dirichlet-energy minimization ("lightning pathfinding" is literal).
- Occam drives self-evaluation: `harmonic-machine.lisp:91` scores cognition as `0.45·simplicity + 0.30·occam + 0.25·interdisciplinary`.
- `:lambdoma` selection gated at the constitutional ratio ≥ 0.72 (`%lambdoma-convergence:167`).
- Invariant `landauer-aware-dreaming`: "erasure has entropy cost — prefer compression over deletion."

**Implication for identification.** The one place harmonia had not yet applied its own epistemology is recovering governing equations — where the DYSCO comparison tempted a **Markov + L1** frame that would *contradict* `constitution.sexp:17-18`. The campaign instead **extends the existing machinery** — the `exp(−size/40)` Solomonoff prior + occam-pass as the program score, `%lambdoma-select` as the candidate selector, the least-action field solve as the variational template — to the new target. "Kolmogorov instead of Markov," in harmonia's own voice. The computable handle is bounded **Levin search** over s-expr programs (K is uncomputable; we measure description length, never claim exact K).

## E. Formal re-verification — measurements (PENDING)

These produce numbers compared to literature; filled as probes run.

- **Invariants (Phase 2) — MEASURED** (`scripts/dev/analysis/invariants.py`; estimator self-checked on canonical RK4 Lorenz → λ_max=+0.906 ✓, so the estimator is trusted). Heat-kernel semigroup / Betti still **PENDING** (need the field actor or a faithful re-impl).

  | System (as built) | λ_max measured | literature | verdict |
  |---|---|---|---|
  | Lorenz Euler, `harmonic-machine.lisp:207` (dt=0.01) | **+1.030** | +0.906 | chaotic ✓, Euler inflates λ ~14% |
  | Lorenz Euler, signalograd base `kernel.rs` (dt=0.008) | **+0.993** | +0.906 | chaotic ✓, Euler inflates λ ~10% |
  | Thomas RK4 **+ `soft_saturate(·,3)`**, `attractor.rs:92` (b=0.208) | **−1.585** | >0 | **NOT chaotic — collapses to a fixed point** |

  **FINDING F1 — the per-step saturation kills Thomas's chaos.** Controlled sweep (3 initial conditions each): bare Thomas (no saturation) is chaotic at both b=0.19 (λ=+0.24) and b=0.208 (λ=+0.21); with the per-step `soft_saturate(out,3.0)` it collapses to a fixed point (λ≈−1.6, trajectory std=0) at *both* b. The saturation Jacobian `sech²(x/3)<1` contracts every step, overwhelming the weak chaotic expansion. So the docstring "smooth … preserves attractor geometry" (`attractor.rs:11`) is **false in the chaotic regime** — the *autonomous* memory-field Thomas is a stable fixed-point system, not the "edge of chaos / 6 coexisting chaotic attractors" of `attractor.rs:39`. **Caveat:** measured autonomous; in situ Thomas is *driven* by the harmonic signal each `:attractor-sync` cycle, so basins may be drive-sustained — the driven case needs a field-actor test before judging functional impact. → flagged for Phase 7.

  **FINDING F2 — Euler drift.** Both Lorenz integrators are genuinely chaotic and bounded, but forward Euler inflates λ_max by 10–14% vs RK4. Harmless for a reservoir basis (it wants rich chaos), but the dynamics are not textbook Lorenz; RK4 removes the drift if fidelity is ever required.
- **D1 symbolic recovery (Phase 4)** — Arm A (DYSCO L1-in-polynomial) vs Arm B (Levin/MDL over s-expr scored by `exp(−size/40)`+occam, `%lambdoma-select`), on clean Lorenz/Thomas ground truth. Metrics: R², dynR², exact-term recovery, description length. **PENDING**
- **D2 well-posedness (Phase 5)** — approximate Solomonoff/compression on the emergent cognitive trajectory (no Markov needed); predictive description length / compression ratio. **PENDING**
- **End-to-end probes (Phase 6)** — deterministic/local suite + 30 Rust tests (serial cargo) + resonance-probe (live LLM, BLOCKED without `OPENROUTER_API_KEY`). **PENDING**
- **Least-action self-rewrite loop (Phase 8)** — fills `:rewrite-plan`'s gated action: identify Θ·Ξ (exact) → s-expr → `sexp-eval` → set one bounded policy parameter toward the minimal-description-length law. Probe asserts ran + bounded + principled. **PENDING**
