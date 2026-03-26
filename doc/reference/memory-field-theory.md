# Memory Field Theory

## Scope

This document is the theoretical foundation for the memory-field subsystem. It captures the research on attractor dynamics, spectral graph theory, information-theoretic pruning, and the physical analogies that ground Harmonia's approach to memory recall.

For architecture and implementation spec, see [memory-as-a-field.md](memory-as-a-field.md).
For the Rust crate reference, see [memory-field-crate.md](memory-field-crate.md).

## Attractor Dynamics and Memory

### Hopfield Energy Landscape

A Hopfield network stores memories as local minima in an energy landscape:

```
E(state) = -½ Σᵢⱼ wᵢⱼ sᵢ sⱼ
```

Weights are set via Hebb's rule: `wᵢⱼ = (1/N) Σμ pᵢμ pⱼμ` where `pμ` are stored patterns. Each pattern becomes a point attractor — a local energy minimum. Recall is gradient descent on this surface: given a noisy or partial input, the network relaxes into the nearest stored pattern.

Key properties:

- Storage capacity: ~0.138N patterns in a network of N neurons (Amit-Gutfreund-Sompolinsky limit).
- Content-addressable: partial input converges to the full stored pattern.
- Spurious states: mixtures of stored patterns can create unwanted attractor basins.
- Retrieval dynamics: overlap with a stored pattern evolves as `m(t+dt) = tanh(β · m(t))`, creating self-amplifying convergence.

Harmonia's Signalograd kernel already implements 32 Hopfield-like memory slots with cosine similarity recall in [kernel.rs](/Users/george/workspace/harmoniis/agent/harmonia/lib/core/signalograd/src/kernel.rs). The memory-field system extends this principle from latent operating modes to the concept graph.

### Hysteresis as Memory Mechanism

A ferromagnetic material remembers its magnetization history because the B-H curve has two stable branches separated by an energy barrier. The system stores not just a value but *which branch it is on*. Traversing from one branch to another requires sufficient energy — the coercive field.

This is exactly the Hopfield energy landscape: switching between stored patterns requires crossing an energy barrier. You cannot casually drift between memories; you must be driven there by sufficient signal.

This is what LLMs lack. LLMs treat memory as flat context — every RAG'd memory sits at the same energy level, equally accessible, equally loud. There is no basin structure, no energy barriers, no hysteresis. A random search query from two months ago has the same activation energy as a core identity fact. The model tries too hard because it has no concept of which attractor basin the current context belongs to.

The memory-field system introduces hysteresis to Harmonia's recall: memories live in attractor basins with energy barriers between them. Weak associations do not trigger basin switches. Only strong, sustained context drives the system across a barrier into a different memory regime.

### Attractor Taxonomy

The memory-field system uses four attractor families, each providing distinct basin geometry:

| Attractor | Equations | Parameters | Basin Structure | Memory Role |
|-----------|-----------|------------|-----------------|-------------|
| Lorenz | dx=σ(y-x), dy=x(ρ-z)-y, dz=xy-βz | σ=10, ρ=28, β=8/3 | 2 wings, butterfly transit | Binary mode switching |
| Thomas | dx=sin(y)-bx, dy=sin(z)-by, dz=sin(x)-bz | b≈0.208 | Up to 6 coexisting attractors | Multi-domain routing |
| Aizawa | dx=(z-b)x-dy, dy=dx+(z-b)y, dz=c+az-z³/3-(x²+y²)(1+ez)+fzx³ | a=0.95, b=0.7, c=0.6, d=3.5, e=0.25, f=0.1 | Sphere + penetrating tube | Depth recall |
| Halvorsen | dx=-ax-4y-4z-y², dy=-ay-4z-4x-z², dz=-az-4x-4y-x² | a=1.89 | 3 interconnected lobes | Interdisciplinary bridging |

### Thomas Attractor: Biological Feedback and Multi-Domain Routing

The Thomas attractor was proposed by Rene Thomas, a biologist studying gene regulatory networks. The cyclic structure `sin(y)→x, sin(z)→y, sin(x)→z` models biological feedback loops where gene A activates B, B activates C, and C activates A, with degradation controlled by parameter b.

The equations are cyclically symmetric under permutation `(x→y→z→x)` — a rare and elegant property. This symmetry means any attractor has rotational copies, and the bifurcation structure inherits this symmetry group.

Bifurcation cascade as b decreases:

- `b > 1`: origin is the sole stable equilibrium.
- `b = 1`: pitchfork bifurcation — splits into two fixed points.
- `b ≈ 0.329`: Hopf bifurcation — stable limit cycle emerges.
- `b ≈ 0.208`: period-doubling cascade triggers chaos. Maximum coexisting attractors (up to 6).
- `b < 0.208`: multiple coexisting attractors emerge via crises; fractal dimension increases toward 3.
- `b = 0`: conservative limit — deterministic fractional Brownian motion.

For memory-field, operating near b ≈ 0.208 (the edge of chaos) provides:

- Many basins = rich repertoire of pre-computed intuitions.
- Flexible boundaries = easy to switch between domains when context demands.
- Parameter b tunable via Signalograd feedback delta, same as existing routing deltas.

The six domains in Harmonia's concept graph (music, math, engineering, cognitive, life, generic) map naturally to the six coexisting Thomas basins.

### Aizawa Attractor: Depth Topology

The Aizawa attractor (properly: Langford system, first published by W.F. Langford in 1984) produces a sphere with a tube-like structure penetrating along one axis. The trajectory wraps around a roughly spherical surface while threading through itself via a narrow channel.

This topology maps to memory depth:

- Shallow memories (daily interactions, recent context) orbit the sphere surface.
- Deep crystallized memories (compressed skills, high-signal patterns) inhabit the tube.
- Recall "dives" through the tube when a query demands depth — the trajectory must transit from sphere to tube, requiring sufficient signal energy.

The system undergoes torus bifurcation: a smooth invariant torus transitions from quasi-periodic to resonant dynamics before breaking up into the chaotic attractor. This transition models the shift from surface scanning to deep retrieval.

### Halvorsen Attractor: Interdisciplinary Bridging

The Halvorsen attractor is cyclically symmetric like the Thomas attractor but with quadratic coupling terms (`-y²`, `-z²`, `-x²`) that create stronger lobe interactions. The three-lobed propeller structure maps to interdisciplinary bridging:

- Each lobe corresponds to a cluster of related domains.
- The interconnections between lobes are the cross-domain edges in the concept graph.
- Lyapunov exponents: L1=0.811, L2=0, L3=-4.626. The positive first exponent confirms chaos; the large negative third confirms strong dissipation.
- Kaplan-Yorke dimension: 2.175, confirming fractal structure.

## Spectral Graph Theory and Recall

### Graph Laplacian

The graph Laplacian is the discrete analog of the wave equation on a continuous surface:

```
L = D - A
```

Where D is the diagonal degree matrix (`D_ii = Σⱼ w_ij`) and A is the weighted adjacency matrix. For Harmonia's concept graph, nodes are concepts and edge weights are co-occurrence counts.

Properties:

- L is symmetric positive semi-definite.
- `L · 1 = 0` — the constant vector is always in the null space (eigenvalue 0).
- All eigenvalues are non-negative: `0 = λ₀ ≤ λ₁ ≤ ... ≤ λₙ₋₁`.
- The number of zero eigenvalues equals the number of connected components.
- The second-smallest eigenvalue λ₁ (the Fiedler value) measures algebraic connectivity.

Physical analogy: if the concept graph is a lattice of masses connected by springs, the Laplacian governs wave propagation. Eigenmodes are the natural standing-wave patterns of the lattice.

### Chladni Pattern Projection

Chladni patterns arise from standing waves on vibrating plates. Sand collects on nodal lines (zeros of the eigenfunction), making the wave structure visible. The 2D pattern is the eigenfunction of a higher-dimensional operator projected down.

Apply this to the concept graph:

- The concept graph is the vibrating plate.
- The incoming signal is the driving frequency.
- The eigenvalues of the graph Laplacian determine which standing-wave patterns exist.
- Each eigenmode is a natural clustering of the graph.
- The signal excites specific eigenmodes → specific nodal patterns appear → memories on anti-nodes (peaks) are recalled, memories on nodes (zeros) are suppressed.

This is the holographic property: the 2D graph surface encodes the full vibrational structure. Different queries excite different eigenmodes, giving frequency-selective recall without explicit categorization.

Connection to atomic physics: Chladni plate patterns are 2D analogs of hydrogen orbital nodal surfaces. Just as electron orbitals are eigenfunctions of a 3D Hamiltonian, concept graph Chladni modes are eigenfunctions of the graph Laplacian.

### Signal Projection and Activation

The mathematical formulation:

```
Eigenmodes:       L · vₖ = λₖ · vₖ
Signal projection: sₖ = ⟨signal, vₖ⟩     (how much the signal excites mode k)
Memory activation: a(i) = Σₖ sₖ · vₖ(i)  (superposition of excited modes at node i)
```

The Fiedler vector (v₁, second eigenvalue) gives the optimal graph bisection — splitting the concept graph into its two most natural clusters. Higher eigenvectors provide finer partitions.

For a concept graph of ~120 nodes, the first 8 eigenvectors capture the dominant clustering structure. This replaces the crude `10*depth + access_count` scoring with a mathematically principled activation pattern.

## Lightning Pathfinding and Field Propagation

### Discrete Potential Field on Graphs

Lightning finds the path of least resistance through air by solving Laplace's equation. A 2024 PLOS ONE paper demonstrated that glow-discharge plasma in microchannels solves mazes by the same principle: electrodes at entrance and exit create a potential difference, plasma explores branches, charge accumulates on surfaces, unnecessary branches are eliminated, and the system converges to the shortest path.

Apply this to memory recall:

```
Source electrode   = current context (high potential on query-concept nodes)
Distributed charges = memory entries on graph nodes
Laplacian solve    = L · φ = b   (potential field across the graph)
Current flow       = Iᵢⱼ = wᵢⱼ(φᵢ - φⱼ)  (through each edge)
Recall             = memories on high-current nodes activate
```

The graph Laplacian is singular (`L · 1 = 0`), so the system is solved with regularization: `(L + εI) · φ = b` where ε = 0.01 anchors the potential to zero-mean.

Conjugate gradient on the sparse Laplacian converges in O(n) iterations for a graph of n nodes. For ~120 nodes with ~160 edges, this is 10-30 iterations, each costing O(|E|) — well within the tick budget.

The key insight: the potential field naturally concentrates current along the shortest, most-connected paths between context and memory. This is mathematically equivalent to the dielectric breakdown model that produces lightning branching patterns.

## Information-Theoretic Pruning

### Kolmogorov Complexity on Graphs

The Kolmogorov complexity K(x) of a string x is the length of the shortest program that produces x. For memory entries on a graph:

```
K(graph) ≈ K(graph \ mᵢ) + K(mᵢ | graph \ mᵢ)
```

If `K(mᵢ | graph \ mᵢ) ≈ 0`: the memory is topologically redundant — other memories already encode the same information via their graph connections. Prune it.

If `K(mᵢ | graph \ mᵢ)` is high: the memory carries unique information that no other path through the graph can reconstruct. Crystallize it.

This makes pruning topological rather than temporal. A two-month-old search query gets pruned not because it is old but because it adds zero topological information. A two-year-old core insight persists because it occupies a unique position in the graph.

### Solomonoff Prior (Existing in Harmonia)

Harmonia already uses Solomonoff-style compression in [compression.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/memory/store/compression.lisp):

```
solomonoff_prior = exp(-new_size / 40)
occam_gate = kolmogorov_ratio ≤ 1.1
```

The memory-field system extends this from content-level compression to graph-level topology. The Solomonoff prior favors shorter descriptions; the topological pruning favors memories that reduce the graph's description length.

### Betweenness Centrality as Irreducibility Proxy

Since Kolmogorov complexity is uncomputable, approximate it via graph topology:

- **High betweenness centrality**: the memory lies on many shortest paths between other memories — it is a bridge, structurally irreducible.
- **Sole community member**: the memory is the only representative of a graph community — removing it destroys an entire cluster's reachability.
- **Low betweenness, high reachability from neighbors**: the memory's information is already accessible via nearby nodes — it is redundant.

This gives a computable proxy for topological irreducibility that directly solves the LLM memory over-eagerness problem: memories that add no structural information to the graph are pruned regardless of how recently they were created.

## Context Collapse (Quantum Measurement Analogy)

### Pre-Query Superposition

Before a query arrives, memory is in superposition — all entries have potential relevance but none is activated:

```
|memory⟩ = Σᵢ αᵢ|mᵢ⟩
```

The amplitudes αᵢ are set by the memory's position in the attractor landscape: higher for memories in frequently-visited basins, lower for memories in rarely-visited basins.

### Query as Measurement Operator

The query acts as a measurement that collapses the superposition:

```
P(recall mᵢ) = |⟨query|mᵢ⟩|²     (Born rule analogy)
```

Implemented via the attractor field:

1. Pre-query: all memories exist as potential energy minima in the attractor landscape.
2. Query injection: the signal perturbs the attractor field (modulates Thomas b, Lorenz σ/ρ/β based on complexity profile).
3. Relaxation: the perturbed field evolves for a few steps → settles into a new attractor basin.
4. Collapse: the basin determines which memories are recalled.
5. Post-measurement: recalled memories' strengths increase; non-recalled memories' strengths stay unchanged.

### Post-Measurement Rule: No Penalty for Silence

The critical design choice: non-recalled memories are not penalized. In quantum mechanics, measurement does not destroy the other states — it just does not select them this time. The LLM memory problem ("tried too hard", "smarmy") comes from systems that treat recall as affirmation and non-recall as irrelevance, creating a positive feedback loop where accessed memories become ever louder.

In the memory-field system, silence is the default state. Activation requires resonance — sufficient signal energy in the right frequency to excite the right eigenmode in the right attractor basin. Everything else stays quiet, not suppressed.

## Connection to Quantum Algorithms on State Machines

Harmonia's harmonic state machine cycles through 9 phases deterministically. A quantum state is a configuration of states in superposition — superpositions are state machines. The harmonic machine's phase transitions (observe → evaluate → balance → project → sync → plan → audit → stabilize) form a cycle that can be analyzed through the lens of quantum walk algorithms:

- **Grover-like amplitude amplification**: the field propagation concentrates activation on relevant memories, analogous to how Grover's algorithm amplifies the amplitude of the target state.
- **Quantum walk on graphs**: the spectral decomposition of the graph Laplacian is the same mathematical object that defines continuous-time quantum walks. The memory activation pattern `a(i) = Σₖ sₖ · vₖ(i)` is the classical shadow of a quantum walk distribution.
- **Adiabatic evolution**: the overnight crystallization cycle (slow compression, basin reshaping) resembles quantum annealing — slowly evolving the energy landscape to find global minima.

These are analogies that inform the design, not claims of quantum computation. The mathematical structures are shared; the implementation is classical.

## Intuition as Pre-Computed Attractor Basins

Evolution does not make humans perform reinforcement learning in real time for every decision. Instead, it shapes attractor basins over generations — the basins are the intuition. A new stimulus falls into a pre-existing basin; the response is immediate.

In Harmonia's memory-field:

- The overnight compression cycle is evolution — it reshapes attractor basins by crystallizing high-signal patterns and pruning redundant ones.
- The real-time cycle is intuition — a new signal falls into the nearest pre-computed basin, activating the memories that resonate.
- Thomas b at the edge of chaos (0.208) provides the maximum number of intuitions (basins) with the most flexible boundaries.

The system does not "try to use" its memories. It vibrates at a frequency set by the incoming signal, and the memories that resonate at that frequency naturally activate. Memories that do not resonate stay silent — not suppressed, just not excited. This is why humans do not feel manipulated by their own memory: intuition is the attractor basin you are already in.

## References to Implementation

| Concept | Implementation | Path |
|---------|---------------|------|
| Hopfield memory slots | Signalograd kernel | `lib/core/signalograd/src/kernel.rs` |
| Lorenz attractor | Signalograd kernel | `lib/core/signalograd/src/kernel.rs` |
| Concept graph | Lisp memory store | `src/memory/store/concept-map.lisp` |
| Crystallization | Lisp compression | `src/memory/store/compression.lisp` |
| Solomonoff prior | Lisp compression | `src/memory/store/compression.lisp` |
| Graph snapshots | Chronicle | `lib/core/chronicle/src/tables/graph.rs` |
| Current recall | Lisp memory store | `src/memory/store/state.lisp` (line 170) |
| 14-dim complexity encoder | Complexity encoder | `lib/core/complexity-encoder/src/` |

## See Also

- [memory-as-a-field.md](memory-as-a-field.md) — architecture and implementation spec
- [memory-field-crate.md](memory-field-crate.md) — Rust crate technical reference
- [signalograd-architecture.md](signalograd-architecture.md) — existing chaos-computing kernel
- [concepts-glossary.md](concepts-glossary.md) — Memory And Scoring terms
