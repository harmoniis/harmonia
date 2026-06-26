# Harmonia ↔ DYSCO — common ground, what each understands, what we verified

Companion to `SCIENTIFIC-VERIFICATION.md`. DYSCO = *"Extracting Governing Equations from Latent Dynamics via Multi-View Contrastive Learning"* (Muratore & Mathis, arXiv 2606.13260). This is the honest comparison the campaign set out to make — grounded in source reading and the D1 experiment (`lib/tools/dynamics-verify`), not assertion.

**One-line thesis.** DYSCO and harmonia both treat cognition/memory as a *latent dynamical system* whose **governing equations** are the object of interest. DYSCO *learns* the basis + encoder + dynamics jointly from noisy observations and pays the price of an **affine-gauge** indeterminacy and a **Markov** modelling assumption. Harmonia *fixes* a chaotic reservoir basis, *owns* its clean latent, and frames recovery as **minimum-description-length (Solomonoff/Kolmogorov)** — which makes exact symbolic recovery possible and drops the Markov assumption, at the cost of relying on the chosen basis being expressive enough.

## 1. Structural mapping

| | DYSCO | Harmonia |
|---|---|---|
| Latent dynamical system | unknown `x_{t+1}=f(x_t)+ε` (Markov, C² diffeomorphism) | signalograd reservoir (Lorenz-driven CTRNN); memory-field attractors (known ODEs); emergent cognitive trajectory (unknown, non-Markov) |
| Observation channel | noisy nonlinear `y=g(x)+ξ`, multi-view | direct, clean latent access (owns its own state) |
| Functional basis `Ξ` | learned, polynomial library | **fixed**: chaotic Lorenz reservoir + golden-ratio harmonics (`lorenz_basis`); s-expr program space for extraction |
| Coefficients `Θ` | contrastive descent (InfoNCE, Muon/AdamW) | local online rules (delta+Oja, chaos-modulated meta-update, Hopfield) |
| Recovery objective | sparsest representative in the affine orbit (L1 over the gauge, eq. 17) | **minimum description length** (`exp(−size/40)` Solomonoff prior + occam, `compression.lisp:35`) |
| Identifiability guarantee | **Theorem 1**: up to affine `(L,b)` | `Lambdoma.v`: recall-reduction bounded+subset (a different, narrower object) |
| Quality metrics | R², dynR² | R², dynR², **description length / Solomonoff prior** |

## 2. Common ground
- **Cognition as a latent dynamical system** to be characterised by its flow field / governing law (Marr's algorithmic level).
- **Chaotic attractors as substrate** — Lorenz appears in *both*: DYSCO's benchmark system, and harmonia's signalograd reservoir basis (σ=10,ρ=28,β=8/3).
- **Symbolic/governing-equation goal** — DYSCO's eq. 3 `fˆ(x)=Θ·Ξ(x)` is *exactly* the form of signalograd's linear readouts; both want an interpretable law, not just a black-box embedding.

## 3. What DYSCO understands that harmonia should heed
1. **Identifiability up to an affine gauge is a fundamental limit** (Theorem 1). Harmonia avoids it by fixing the basis — but DYSCO's theory tells us the *price*: harmonia's recovery is only as good as its basis (see §5). Harmonia has no gauge concept; if it ever *learns* a basis, the gauge returns.
2. **Multi-view consistency as a denoising mechanism** (`1/√V`). Harmonia is **single-view / quenched** — it observes each cognitive state once. This is the one ingredient harmonia lacks and would need before identifying its *emergent* (noisy, non-Markov) dynamics is meaningful (see Phase 5 / D2).
3. **The "trajectory-recoverable-but-dynamics-not" regime** (their Fig 3b): when noise dominates, the latent trajectory is still recoverable while the flow field is not. A real diagnostic for harmonia's emergent-dynamics case.

## 4. What harmonia has that DYSCO lacks — *verified*, not asserted
1. **Exact symbolic recovery in the clean / known-basis regime.** DYSCO's own Appendix C concedes its symbolic step leaves spurious terms (L1 thresholding within the gauge, "relies on exact term cancellations"). Harmonia's fixed named basis + s-expr + description-length objective removes both the gauge and the L1 proxy → **exact** recovery. *Evidence (D1):* on Lorenz both methods tie exactly; this is the soundness control, not the contribution.
2. **A genuine program language + evaluator** (`sexp-eval`) lets harmonia *search and run* candidate laws — including **transcendental primitives** a fixed polynomial library cannot express. *Evidence (D1):* on harmonia's own **Thomas** attractor (`ẋ=sin y−bx`), a polynomial basis fails (30 spurious terms, dynR²=0.969) while a library with `sin` recovers `(- (sin y) (* 0.19 x))` exactly (dynR²=1.0); the description length collapses 585→93 chars and the `exp(−size/40)` prior ranks the exact law ~10⁵× higher. **This is the constructive contribution back to DYSCO:** their weak symbolic step becomes exact once gauge + L1 are replaced by a known basis + MDL — and a chaotic-reservoir or transcendental basis recovers laws a polynomial library structurally cannot.
3. **Online, local, auditable learning** (no batch contrastive optimisation) — CPU-cheap, runs continuously, every learned effect inspectable (`signalograd-status`, chronicle).
4. **A non-decaying topological-flux (A-B) ingredient** in basin dynamics (`topology.rs`) — no DYSCO analog. Novelty; whether it *helps* is an ablation, not a claim.

## 5. The deepest distinction — Markov vs Kolmogorov (constitutional, not a bolt-on)
DYSCO inherits a **Markov** generative model. Harmonia's epistemology is **Solomonoff/Kolmogorov/least-action**, and it is *constitutional*: `constitution.sexp:17` `"Compression is intelligence. Solomonoff prior exp(-size/40)."`; `:18` `"Laplacian field solve finds shortest paths"` (the field `(L+εI)φ=b` literally minimises Dirichlet energy — least action). So harmonia recovers a governing law as the **shortest s-expr program** that reproduces a trajectory — the path of least action in *program* space — which is well-posed for *any* computable sequence, **dropping** DYSCO's Markov + diffeomorphism requirements. This is what makes the emergent, externally-forced (LLM-driven), non-Markov cognitive trajectory a well-posed target for compression (Phase 5) where DYSCO's Theorem 1 cannot apply.

**Honest caveat:** Kolmogorov complexity is uncomputable; harmonia uses bounded description-length search (the `exp(−size/40)` prior + occam over s-expr size) as the computable proxy. We measure description length; we never claim exact `K`.

## 6. Did DYSCO misunderstand anything? — No.
Stated plainly: DYSCO is sound and solved a **harder** problem (unknown basis + noisy multi-view observations) than harmonia faces (own clean latent + fixed basis). Harmonia's "direct latent access" **sidesteps**, it does not beat, the inverse problem DYSCO exists to solve. The only fair head-to-head is the clean / known-basis regime, and there the result is **constructive**: harmonia shows how DYSCO's acknowledged-weak symbolic step can become exact, and that a richer (transcendental / chaotic) basis recovers laws the polynomial basis cannot. No manufactured flaw.

## 7. What harmonia should adopt from DYSCO
- **Multi-view consistency** — re-observe the same cognitive state under resampled telemetry noise and average, to denoise the *emergent* dynamics before attempting to identify them. This is the prerequisite (per §3.2 / Phase 5) for lifting harmonia's exact-recovery result from the clean known-basis regime to its own real, noisy cognition.
