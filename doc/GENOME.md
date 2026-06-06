# Harmonia as a Living Organism — Genome, Epigenetics, Immune System

**Thesis.** The agent is one organism. Every Lisp component is a *cell*; every cell carries the
**genome** and expresses it. Behavior is not hardcoded per cell — it is the genome's *gene expression*,
modulated by *epigenetic* marks written by lived experience, **clamped by the genome's own bounds**,
defended by an **immune system** that is itself expressed from the genome. The LLM is not the organism —
it is the **entangled metabolic engine** the genome drives and that, in turn, writes back the epigenetic
marks. Life is that loop running continuously, bounded by the germline, healed toward it.

Rust crates and tools are the **environment/body** the organism acts through — we do not propagate the
genome into them. The genome and epigenetic behavior live in **all Lisp cells**.

---

## 1. What exists today (verified) — and why it isn't yet alive

- **Genome skeleton exists** (`src/dna/dna.lisp` `*dna*`): identity, 9 genes, 12 constraints, 8 bounds,
  foundation. Foundation is live (wired to harmonic dynamics). Constraints partly enforced (6/12).
- **The genome is mostly inert as a control surface:**
  - **Genes are never expressed.** `dna-gene` is defined but never called; all 9 genes are invoked by
    hardcoded function name → changing a gene in the genome has **zero effect**.
  - **Bounds are never read.** `dna-bound`/`dna-clamp-to-bound` are defined but never called; every
    epigenetic mark (signalograd deltas, meditation weights, decay-λ) is clamped by **hardcoded module
    limits**, so the germline does not constrain the soma.
  - **The genome is incomplete.** `:laws` and `:prime-directive` are *referenced* (compression,
    concept-map, harmonic-machine) but **absent** from `*dna*` → `(getf *dna* :laws)` is NIL. The
    organism has no stated prime directive or laws in its own germline.
- **Epigenetics works but is fragmented.** Signalograd deltas, swarm success-rates, meditation edges,
  logistic-r are bounded/heritable/reversible — good — but each lives in its own store with its own
  limits; there is no single epigenome and no genome-level bound. **REPL fluency (`*repl-model-perf*`)
  is not persisted** → the agent forgets which models can drive it on every restart.
- **The immune system only observes.** Security posture (`:nominal/:elevated/:alert`) is computed but
  **never gates behavior**. Injection dissonance is computed but tainted signals are not quarantined.
  The restricted evaluator (`sexp-eval`) is the one real membrane (genome-bounded). Self-healing
  (recovery/phoenix/ouroboros) is reactive only.
- **No cell carries the genome.** Each port hardcodes its parameters; there is no per-cell expression.

---

## 2. The perfect genome (the germline) — five chromosomes

`*dna*` becomes the **complete, single source** of behavioral specification. Immutable at runtime
(germline); changed only by deliberate evolution.

1. **IDENTITY** — creator, spirit, **prime-directive**, **laws** (the constitution). The "self" the
   immune system protects. (Adds the two missing chromosomes.)
2. **GENES** — capability → expressing function. Cells call capabilities through `(dna-express-gene :k …)`,
   so the genome is the live dispatch table. Genes are the *only* capabilities the LLM may invoke
   (the homoiconic primitive frame is generated *from* the genes).
3. **REGULATORY (bounds)** — for every tunable parameter, the `(min . max)` range within which
   epigenetics may drift. **Every epigenetic mark is clamped to its genome bound** — the germline
   constrains the soma. (Absorbs the W2 meditation ceilings, decay-λ, signalograd limits, etc.)
4. **FOUNDATION** — the mathematical laws (vitruvian, lorenz, thomas, kolmogorov, lambdoma…). Already live.
5. **IMMUNE** — self-identity (creator/PGP), boundary rules, and the **posture → behavior** map
   (what `:elevated`/`:alert` actually *do*). The immune genes.

---

## 3. Epigenetics — the germline bounds the soma, clamped *at write*

The property that was missing is **bounded drift**: epigenetic marks must never exceed the genome's
`:bounds`. The correct, performant way to enforce this is **not** a read-time accessor on every
parameter (that would route every selection/scoring/recall read through multi-store lookups — the same
pervasive hot-path tax that hung boot for 8 minutes). Instead, **clamp at the handful of cold-path sites
where each mark is already written/applied**, replacing their *hardcoded* limits with the genome's bound:

```
mark_new = dna-clamp-to-bound( PARAM, mark_old ⊕ experience )
```

The sites already exist and already clamp to hardcoded constants — we point them at the genome:
`%reinforce-weight` / `%decay-concept-edges` (meditation), `%signalograd-sanitize-proposal` (kernel
deltas), the swarm `%avg-update` (success-rate). This gives 100% of "the germline bounds the soma" at
~5 cold-path sites, zero hot-path cost, reusing existing code. Marks remain **heritable** (persisted)
and **reversible** (decay / dream). The W2 meditation ceilings/decay-λ move into the genome `:bounds`.

**Heritability fix.** Persist `*repl-model-perf*` (REPL fluency) like the other marks, so the agent's
learned competence at driving each LLM survives restart — the most load-bearing epigenetic memory for a
homoiconic agent.

---

## 4. The immune system — expressed from the genome, and it RESPONDS

Self/non-self defense, integrity, and self-healing, driven by the IMMUNE chromosome:

- **Membrane (already real):** `sexp-eval` restricts what code may run (genome-bounded rounds/results,
  denied operators). Keep; express its limits from the genome.
- **Self/non-self:** signals carry `:taint`. Non-self (external/tool-output) input that trips injection
  dissonance is **quarantined** at the boundary, not merely logged.
- **Immune RESPONSE (new — the missing piece):** the security posture **gates behavior**, expressed from
  the genome's `:immune` posture map. `:elevated` tightens (lower chaos-max, fewer swarm subagents,
  stricter eval, no risky rewrites); `:alert` goes defensive (restrict `exec`/`datamine`, quarantine
  tainted input, minimal surface). Posture is itself an epigenetic mark, bounded and reversible
  (it relaxes back toward `:nominal` as threat decays).
- **Self-healing toward the germline:** recovery/phoenix/ouroboros restore failed cells; homeostasis is
  "converge back to the genome's specified state." A periodic genome-integrity check (germline intact,
  self-identity valid) on the harmonic cadence.

---

## 5. Genome → LLM: the entangled life-process

The genome **controls** the LLM; the LLM **animates** the genome:

- **Expression (genome → LLM):** the DNA-composed prompt carries identity + prime-directive + laws; the
  REPL frame teaches exactly the **genes** as callable primitives; constraints bound the interaction;
  epigenetic model selection picks *which* LLM, modulated by experience and gated by the immune posture.
- **Animation (LLM → genome):** the LLM's eval outcomes write epigenetic marks (fluency, success-rate,
  meditation, harmony) that modulate the next expression.
- **Life = the loop:** genome expresses → LLM acts → experience marks the epigenome (within bounds) →
  next expression differs. The agent is *alive* while this runs continuously, *bounded* by the germline,
  *defended* by the immune system, *healing* toward its identity. The LLM is interchangeable
  ("harness any model") precisely because it is the metabolic engine, not the self — the self is the genome.

---

## 6. What we build (Lisp-only; each guarded by a probe)

G1. **Complete the genome:** add `:prime-directive`, `:laws`, and the `:immune` chromosome (self + posture
   map) to `*dna*`. The genome becomes whole.

G2. **Genes made load-bearing without indirection:** keep the direct calls (compile-time safety), but
   (a) **generate the REPL primitive frame from `:genes`** so the genome literally defines what the LLM may
   call, and (b) have `dna-valid-p` assert every gene symbol is `fboundp` so a broken gene mapping fails
   loudly at boot. No runtime `funcall`-by-symbol hot path.

G3. **Clamp-at-write genome bounds:** move the W2 meditation ceiling/decay-λ/edge-count into the genome
   `:bounds`, and make the mark-write sites (`%reinforce-weight`, `%decay-concept-edges`,
   `%signalograd-sanitize-proposal`, swarm `%avg-update`) clamp via `dna-clamp-to-bound`. The germline now
   constrains the soma at the cold path. No read-time accessor.

G4. **Immune response:** posture → behavior gating from the `:immune` map (chaos-max, swarm fan-out, eval
   strictness, risky-op refusal); taint quarantine at the boundary; posture as a bounded, reversible mark.

G5. **Heritable fluency:** persist/restore `*repl-model-perf*` as an epigenetic mark.

G6. **Genome homeostasis probe + multi-use-case tests** (next section).

---

## 7. Verification (multiple use cases)

`scripts/dev/genome-probe.lisp` (+ harness drives) asserting the organism is alive and unified:

- **Completeness:** genome has identity/genes/bounds/foundation/immune + prime-directive + laws (non-NIL).
- **Gene expression:** swapping a gene's function symbol changes the dispatched behavior (genome is live).
- **Germline bounds the soma:** an epigenetic mark pushed past its genome bound is clamped to the bound
  (e.g. meditation ceiling, decay-λ, a signalograd delta) — read via `dna-express`.
- **Immune response:** raising injection events drives `:elevated`/`:alert` AND posture measurably
  tightens behavior (lower effective chaos-max / swarm fan-out / refuses a risky rewrite); tainted input
  is quarantined.
- **No autoimmunity (the critical negative test):** under `:elevated`, **legitimate work still flows**
  (a normal turn still answers), and posture **provably relaxes back to `:nominal`** as threat decays —
  the organism must never strangle itself on a false positive.
- **Heritability:** record fluency, "restart" (reload), fluency survives and steers selection.
- **Outcome, not just mechanism:** a real multi-turn task and a recall task are **as good or better**
  after all the wiring (the agent answers and remembers well) — architecture must not regress behavior.
- **The living loop:** genome → LLM (a real TUI turn) → epigenetic mark written → next expression differs;
  run diverse use cases (build, recall, injection, idle) and assert the organism expresses, learns,
  defends, and heals — all within germline bounds.
