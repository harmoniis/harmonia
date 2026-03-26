# Memory As A Field

## Summary

The memory-field subsystem replaces linear search-based recall with dynamical field propagation on the concept graph. Memory becomes a potential field; recall becomes relaxation into attractor basins; relevance becomes resonance, not substring matching.

Its job is to bridge Signalograd's attractor dynamics with the Lisp memory store, giving the system frequency-selective recall, hysteresis-protected basin switching, and topological pruning — all without changing the 4-class memory model or breaking conductor policy boundaries.

For theoretical foundations, see [memory-field-theory.md](memory-field-theory.md).
For the Rust crate reference, see [memory-field-crate.md](memory-field-crate.md).

## The Problem

| Layer | What Exists | What's Missing |
|-------|------------|----------------|
| Lisp memory | 4-class store, concept graph, crystallization | Retrieval is substring match + access count — no dynamics |
| Signalograd | 32 Hopfield slots, cosine recall, Lorenz basis | Disconnected from Lisp concept graph entirely |
| Chronicle | SQL snapshots, graph decomposition, CTE traversal | Used for observability, not active recall |
| Concept graph | Nodes, edges, domains, interdisciplinary bridges | Exists statically — no field propagation through it |

Current retrieval path: `query → substring match → sort by 10*depth + access_count`. That is linear search with a heuristic, not a dynamical system finding its basin.

See: [state.lisp](/Users/george/workspace/harmoniis/agent/harmonia/src/memory/store/state.lisp), `memory-layered-recall` (line 170).

## What Memory-Field Is

- A field propagation engine on the concept graph.
- A bridge between Signalograd attractor dynamics and Lisp memory recall.
- A spectral decomposition framework for frequency-selective retrieval.
- A topological pruning system based on Kolmogorov complexity, not temporal age.
- A Rust kernel accessed via IPC, consumed by Lisp orchestration.

## What Memory-Field Is Not

- Not a replacement for the 4-class memory store (soul, skill, daily, tool).
- Not a second Signalograd — it does not learn online from telemetry.
- Not a vector database or embedding store.
- Not allowed to bypass conductor policy or memory class boundaries.
- Not allowed to mutate memory entries directly.

Lisp remains the semantic boundary. The Rust kernel computes field dynamics; Lisp interprets, clamps, and applies the result.

## Architecture

### Field Propagation

Replaces substring recall with potential field solving on the concept graph.

The concept graph already has nodes and weighted edges. Instead of searching it, solve a potential field on it — like lightning finding the path of least resistance through a maze:

```
Source electrode   = current context (query concepts set as high potential)
Distributed charges = memory entries on graph nodes
Laplacian solve    = L · φ = b   (L = graph Laplacian, b = source vector)
Current flow       = Iᵢⱼ = wᵢⱼ(φᵢ - φⱼ)
Recall             = memories on high-current nodes activate
```

Each incoming signal has a complexity profile (the existing 14-dim encoder). Map that profile to concept-graph nodes via domain classification (already exists). Set those nodes as source potentials. Solve the discrete Laplacian via conjugate gradient. Memories on high-potential nodes get recalled; others stay dormant.

Complexity: O(n) per sparse solve. For ~120 nodes with ~160 edges, 10-30 CG iterations converge — compatible with the tick loop.

### Multi-Attractor Basin Assignment

Bridges Signalograd's attractor dynamics to the concept graph.

Each concept-graph cluster maps to an attractor basin. When the system is in basin B, only memories in basin B are preferentially active. Switching basins requires crossing an energy barrier — the hysteresis that prevents weak associations from polluting recall.

| Attractor | Basin Structure | Memory Role |
|-----------|-----------------|-------------|
| Lorenz (existing in Signalograd) | 2 wings, butterfly transit | Binary mode switching (work/creative) |
| Thomas (cyclically symmetric) | Up to 6 coexisting attractors, controlled by single param b | Multi-domain routing — each domain maps to one Thomas attractor. Cyclic symmetry = smooth transitions |
| Aizawa (sphere + tube) | Surface shell + penetrating channel | Depth recall — shallow memories orbit the surface, deep crystals inhabit the tube |
| Halvorsen (3-lobed propeller) | Three interconnected lobes | Interdisciplinary bridging — each lobe maps to a domain cluster |

Thomas attractor at b ≈ 0.208 (edge of chaos): maximum coexisting attractors with most flexible basin boundaries. The b parameter is tunable via Signalograd feedback delta.

### Chladni Projection

Spectral graph theory applied to the concept graph for frequency-selective recall.

```
Graph Laplacian:    L = D - A   (degree matrix minus adjacency)
Eigenmodes:         L · vₖ = λₖ · vₖ
Signal projection:  sₖ = ⟨signal, vₖ⟩   (how much the signal excites mode k)
Memory activation:  a(i) = Σₖ sₖ · vₖ(i)  (superposition of excited modes at node i)
```

Nodes with high activation get recalled. This gives:

- **Frequency-selective recall** — different query types excite different eigenmodes.
- **Natural clustering** — the Fiedler vector (2nd eigenvalue) is the optimal graph bisection.
- **Holographic property** — the 2D graph surface encodes the full vibrational structure.

This replaces `10*depth + access_count` scoring with a mathematically principled activation pattern.

### Kolmogorov Topological Pruning

Replaces temporal decay with topological irreducibility.

```
K(graph) ≈ K(graph \ mᵢ) + K(mᵢ | graph \ mᵢ)
```

If `K(mᵢ | graph \ mᵢ) ≈ 0`: the memory is topologically redundant — prune it.
If `K(mᵢ | graph \ mᵢ)` is high: the memory carries unique information — crystallize it.

Approximated via betweenness centrality and community membership on the concept graph. A memory is redundant if its concept nodes are all reachable via short paths from other memories. A memory is irreducible if it occupies a unique position in the graph topology.

This solves the "2-month-old search query" problem directly: pruning by topological redundancy, not by age.

### Context Collapse

Query as measurement operator collapsing memory superposition.

1. **Pre-query**: all memories exist as potential energy minima in the attractor landscape.
2. **Query injection**: signal perturbs the attractor field (modulates Thomas b, Lorenz parameters).
3. **Relaxation**: perturbed field evolves for a few steps → settles into a new attractor basin.
4. **Collapse**: the basin determines which memories are recalled.
5. **Post-measurement**: recalled memories' strengths increase; non-recalled memories stay unchanged — not penalized.

The critical rule: do not penalize non-recalled memories. Silence is the default. Activation requires resonance.

### Actor-Propagated Recall

Async wave propagation via the actor system.

1. Signal arrives → Gateway actor.
2. Gateway extracts concept signature → posts to MemoryFieldActor.
3. MemoryFieldActor injects source potentials on concept graph.
4. MemoryFieldActor propagates field (1-3 Laplacian iterations).
5. MemoryFieldActor posts activated memories to Conductor actor.
6. Meanwhile, Conductor has already started reasoning with existing context.
7. Recalled memories arrive as late-binding context — enhancing, not replacing.

This is the reactor pattern: the tick loop does not block on memory recall. The wave propagates asynchronously. If recall completes before the LLM response, it enhances the context. If not, the response proceeds without it — and the recall result seeds the next cycle's field state.

## Implementation Phases

| Phase | Deliverable | Dependencies | Key Files |
|-------|-------------|-------------|-----------|
| 1 | Graph Laplacian recall engine | chronicle graph.rs | `lib/core/memory-field/src/graph.rs`, `field.rs` |
| 2 | Attractor dynamics (Thomas, Aizawa, Halvorsen) | none | `lib/core/memory-field/src/attractor.rs` |
| 3 | Eigenmode recall (Chladni) | Phase 1 Laplacian | `lib/core/memory-field/src/spectral.rs` |
| 4 | Kolmogorov topological pruning | Phase 1 graph topology | `src/memory/store/compression.lisp` extension |
| 5 | Async MemoryFieldActor | Phases 1-3, runtime actors | `lib/core/runtime/src/actors.rs` |
| 6 | Hysteresis state tracking | Phase 2 basins | `lib/core/memory-field/src/basin.rs` |

## Physical Analogy Map

| System Component | Physical Analog | Function |
|-----------------|-----------------|----------|
| Concept graph | Crystal lattice | Structure |
| Graph Laplacian | Wave equation on lattice | Propagation |
| Eigenmodes | Chladni standing waves | Natural clustering |
| Attractor basins | Ferromagnetic domains | State memory (hysteresis) |
| Field solve | Lightning pathfinding | Optimal recall paths |
| Kolmogorov pruning | Minimum energy principle | Compression |
| Thomas parameter b | Temperature | Controls flexibility vs stability |
| Signal injection | Quantum measurement | Context collapse |
| Overnight crystallization | Annealing | Basin reshaping |
| Actor propagation | Wave propagation speed | Async efficiency |

Everything follows from one principle: memory is a field, not a database. Recall is relaxation into attractors, not search through records. Relevance is resonance, not matching.

## Policy Boundaries

Memory-field may:

- emit recall activation weights
- propagate fields on the concept graph
- cache spectral decompositions
- advise on topological pruning candidates
- report basin status and eigenmode structure

Memory-field may not:

- bypass conductor policy
- directly mutate memory store entries
- override security kernel invariants
- operate outside actor mailbox protocol
- weaken memory class boundaries

## Activation Scoring

The final activation score for each concept node combines four signals:

```
basin_factor = if node_basin == current_basin { 1.0 } else { 0.15 }

activation[i] = clamp(
    0.40 × normalize(field_potential[i])
  + 0.30 × normalize(eigenmode_activation[i])
  + 0.20 × basin_factor
  + 0.10 × normalize(access_count[i]),
  0.0, 1.0)
```

Nodes not in the current attractor basin receive a 0.15 multiplicative factor — they are not silenced, but strongly de-prioritized. Only a basin switch (requiring coercive energy) promotes them.

## How Text Becomes Field Energy

The encoding path from raw text to field activation:

1. **Tokenization** (`%split-words` in `state.lisp`): Normalize to lowercase, strip non-alphanumeric, split on whitespace, filter words under 3 characters, remove 26 stopwords. "How does the Rust compiler work?" → `("rust" "compiler" "work")`.

2. **Node creation** (`%upsert-concept-node` in `concept-map.lisp`): Each word becomes a concept node with domain classification. "rust" → `:engineering`, "compiler" → `:generic`. Nodes accumulate reference counts and entry ID lists.

3. **Edge creation** (`%upsert-concept-edge`): All word pairs within the same entry form co-occurrence edges. Words that frequently appear together build strong edges. This is how topic clusters emerge organically.

4. **Graph construction** (`build_graph` in `graph.rs`): The Lisp concept graph is serialized and sent to the Rust field engine. Nodes and edges become a Compressed Sparse Row matrix. The graph Laplacian L = D - A is computed.

5. **Spectral decomposition** (`spectral_decompose`): First K eigenvectors of the Laplacian are computed and cached. These are the Chladni modes — standing wave patterns on the concept graph.

6. **Field activation on query**: Query words set source potentials. Conjugate gradient solves L·φ = b. Eigenmode projection produces Chladni activation. Basin filter applies hysteresis. The result is a ranked list of concept nodes with activation scores.

7. **Entry resolution**: Activated concept nodes carry entry IDs linking back to the Lisp `memory-entry` structs. The caller receives the same `memory-entry` objects as the old substring recall — the interface is unchanged.

### What Is Preserved and Lost

**Preserved**: topic clustering (co-occurrence edges), domain structure (6 domains), temporal evolution (graph grows with new entries), cross-domain bridges (interdisciplinary edges).

**Lost**: word order ("rust compiler" = "compiler rust"), compound phrases ("machine learning" → two separate concepts), negation ("not working" drops "not"), numerical values (non-alphanumeric stripped).

See [memory-field-observations.md](memory-field-observations.md) for proposals to address these limitations (P2: phrase-aware extraction, P3: semantic similarity edges).

## Multi-Modal Memory: Beyond Text

The memory field operates on a weighted graph. It does not know or care what produced the nodes. Any data that can be decomposed into discrete features can enter the concept graph.

### What Works Today

- **Text**: Full support via `%split-words` → concept nodes.
- **Structured JSON**: Detected as `message.structured`, text content analyzed.
- **Code**: Keywords detected by the 14-dim complexity encoder.
- **Audio**: Transcribed to text via Whisper (Groq/OpenAI), then enters the text pipeline.

### What Needs New Encoders

| Data Type | Encoder Needed | Concept Vocabulary | Domain |
|-----------|---------------|-------------------|--------|
| Music audio | Pitch/rhythm/timbre extractor | "A4", "major", "4/4", "violin" | `:music` |
| Images | Object/scene/color detector | "cat", "outdoor", "blue" | new `:visual` |
| Structured data | Schema-aware field extractor | Column names, value ranges | `:engineering` |
| Sensor data | Time-series feature extractor | "rising", "periodic", "anomaly" | `:life` or new |

### The Principle

The field is modality-agnostic. The encoding step is the only modality-specific component. To add music memory:

1. Create `lib/core/audio-encoder/` — extract musical features as concept nodes.
2. Register audio concepts in the domain vocabulary.
3. Store audio memory entries with `(:type :audio :features (...))` content.
4. The field, attractors, basins, and hysteresis work unchanged.

The holographic property applies: the 2D concept graph surface encodes high-dimensional information from any modality, projected onto a shared topology where Chladni modes don't distinguish between a "harmony" concept from text and a "harmony" concept from detected audio intervals.

## Implementation Map

| Boundary | File | Purpose |
|----------|------|---------|
| Rust crate | `lib/core/memory-field/` | Graph Laplacian, spectral cache, attractor dynamics, activation engine |
| Lisp port | `src/ports/memory-field.lisp` | IPC wrapper for field recall |
| Recall replacement | `src/memory/store/state.lisp` | `memory-layered-recall` dispatches to field when available |
| Concept graph source | `src/memory/store/concept-map.lisp` | Source of graph data serialized to Rust |
| Chronicle graph store | `lib/core/chronicle/src/tables/graph.rs` | Persistent graph snapshots |
| Signalograd kernel | `lib/core/signalograd/src/kernel.rs` | Lorenz attractor state (read by memory-field for basin context) |
| Harmonic machine | `src/core/harmonic-machine.lisp` | Graph push in `:observe`, basin read in `:attractor-sync` |
| Actor integration | `lib/core/runtime/src/actors.rs` | MemoryFieldActor following ComponentMsg pattern |
| Dispatch registration | `lib/core/runtime/src/dispatch.rs` | `"memory-field"` component routing |

## See Also

- [memory-field-theory.md](memory-field-theory.md) — theoretical foundations
- [memory-field-crate.md](memory-field-crate.md) — Rust crate technical reference
- [memory-field-observations.md](memory-field-observations.md) — test results, basin state insight, improvement proposals
- [signalograd-architecture.md](signalograd-architecture.md) — existing chaos kernel
- [concepts-glossary.md](concepts-glossary.md) — Memory And Scoring terms
- [system-map.md](system-map.md) — full system diagram with data encoding flow
