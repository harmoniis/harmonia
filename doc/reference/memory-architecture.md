# Memory Architecture -- Complete System Reference

## Scope

This document is the master reference for Harmonia's complete memory system. It covers all four memory layers, their interactions, persistence model, signal flow, and idle-time processing. Every component is traced to its implementation file.

For theoretical foundations, see [memory-field-theory.md](memory-field-theory.md).
For the Rust crate reference, see [memory-field-crate.md](memory-field-crate.md).
For the signalograd kernel, see [signalograd-architecture.md](signalograd-architecture.md).

## Overview

Harmonia's memory is a four-layer hierarchy. Each layer operates at a different level of abstraction. Information flows through all layers on every interaction.

```
L0: Boot Memory     (DNA/genesis seeds — immutable identity)
L1: Memory Field    (dynamical system — the living brain)
L2: Persistent Store (palace .md verbatim + concept entries — searchable GBs)
L3: Datamining      (cross-node cross-tool mining — flows into L2)
```

Signalograd is NOT a memory layer — it is system evolution state (119 dynamic weights learned by Hebbian inference). Chronicle is NOT a memory layer — it is a system operational log (SQLite, queryable for telemetry).

The common constant across layers: the Hebbian learning rule `dw = eta * error * signal`. It appears in meditation (L2), field dreaming (L1), and signalograd weight learning (system state).

The common structure: convex combination `alpha * primary + (1 - alpha) * secondary + epsilon`. It appears in scoring weights, projection deltas, and the vitruvian triad.

## L0: Boot Memory (DNA / Genesis Seeds)

Immutable identity layer. Loaded once at boot from `src/dna/dna.lisp`. Creates `:soul` entries at depth 2+ (near-permanent). Includes: creator identity (PGP fingerprint), spirit, prime directive, laws, foundation concepts (vitruvian, kolmogorov, solomonoff, lorenz, thomas, chladni, lambdoma). Idempotent — re-seeding the same DNA produces no duplicates (content-hash dedup in Chronicle).

Implementation: `src/dna/dna.lisp` (`memory-seed-soul-from-dna`).

## L1: Memory Field (Dynamical System)

The living brain. Operates on the concept graph using graph Laplacian field propagation, attractor dynamics, and topological invariants. Recall is relaxation into attractor basins, not keyword search. See detailed sections below.

## L2: Persistent Verbatim Store

Fusion of two subsystems: the Memory Palace (verbatim .md files on disk) and the Concept Memory Store (semantic structure backed to Chronicle). Together they form the searchable long-term storage across GBs of data.

### L2a: Memory Palace (Verbatim)

Stores content exactly as received -- no summarization, no lossy compression. The palace is the authoritative source of truth for all verbatim content. Persists as .md files under `nodes/<label>/memory/palace/`.

### Graph Structure

The palace is a directed graph with three node types forming a containment hierarchy:

| Node Kind | Role | Example |
|-----------|------|---------|
| Wing | Top-level domain partition | "engineering", "life", "music" |
| Room | Topic within a wing | "rust-memory-field", "daily-2026-04-12" |
| Tunnel | Cross-domain bridge | Connects 2+ Wings |

Wings contain Rooms. Rooms contain Drawers (the actual content). Tunnels bridge Rooms across Wings, enabling cross-domain recall.

Edge kinds: `Contains`, `RelatesTo`, `Bridges`, `Temporal`, `Causal`.

Domains: `Music`, `Math`, `Engineering`, `Cognitive`, `Life`, `Generic`, `System`.

### Drawers

Each drawer stores verbatim content with provenance metadata:

```
Drawer {
    id: u64,
    content: String,        -- verbatim, never modified after creation
    source: DrawerSource,   -- provenance tag
    room_id: u32,           -- containing room
    chunk_index: u16,       -- ordering within a multi-chunk file
    created_at: u64,        -- epoch milliseconds
    tags: Vec<String>,      -- semantic labels
}
```

Four drawer sources track provenance:

| Source | Origin | Fields |
|--------|--------|--------|
| Conversation | User interaction | session_id |
| File | Ingested file | path, hash |
| Datamining | Terraphon extraction | lode_id, node_label |
| Manual | Direct insertion | (none) |

### AAAK Compression (Adaptive Alphabet Koder)

Non-destructive compression that preserves original drawers and adds a compressed layer alongside them. Target compression ratio: ~30x.

Mechanism:
- LRU codebook with 256 entries: frequent entities map to 2-character codes.
- Codebook persisted in config-store as JSON.
- `compress_aaak(drawer_ids)` combines content from specified drawers, counts word frequencies, registers frequent entities into the codebook, and emits a compressed `AaakEntry`.

The AaakEntry contains: entity codes with counts, topic label, weight, flags, and source drawer IDs (backlinks to verbatim content).

### Tiered Context Retrieval

Four retrieval tiers provide progressive context injection -- from minimal identity to full-text search:

| Tier | Name | What It Returns | Token Cost |
|------|------|-----------------|------------|
| L0 | Identity | Wing node labels only | Minimal |
| L1 | Essential | Most recent drawers by room (sorted by created_at, limited to ~15) | Low |
| L2 | Filtered | Domain-filtered drawers (rooms matching a specific domain) | Medium |
| L3 | Deep | Full-text search across all drawers (up to ~30 results) | High |

The conductor selects the appropriate tier based on context window budget and query complexity.

### Implementation

| File | Purpose |
|------|---------|
| `lib/core/mempalace/src/lib.rs` | PalaceState, init, persist, health_check |
| `lib/core/mempalace/src/graph.rs` | KnowledgeGraph, GraphNode, GraphEdge, NodeKind, Domain, EdgeKind, add_node, add_edge, find_tunnels |
| `lib/core/mempalace/src/drawer.rs` | Drawer, DrawerSource, DrawerStore, file_drawer, get_drawer, search_drawers |
| `lib/core/mempalace/src/aaak.rs` | AaakEntry, compress_aaak, codebook_lookup, codebook_register |
| `lib/core/mempalace/src/codebook.rs` | AaakCodebook (LRU 256-entry codebook, JSON serialization) |
| `lib/core/mempalace/src/compress.rs` | Drawer-level compression utilities |
| `lib/core/mempalace/src/layers.rs` | context_l0, context_l1, context_l2, context_l3 |
| `lib/core/mempalace/src/query.rs` | query_graph, Traversal |
| `lib/core/mempalace/src/sexp.rs` | S-expression parsing for palace IPC |
| `src/ports/mempalace.lisp` | Lisp port -- IPC bridge to the Rust crate |
| `lib/core/mempalace/tests/integration.rs` | Integration tests |

### Persistence

All palace state persists to disk under `nodes/<label>/memory/`:

```
nodes/<label>/memory/
├── palace/
│   ├── index.sexp                    ← Graph structure (homoiconic, Lisp-readable)
│   └── <wing>/
│       └── <room>/
│           ├── 000001.md             ← Verbatim drawer (YAML frontmatter + content)
│           ├── 000002.md
│           └── ...
└── codebook.json                     ← AAAK entity-to-code mappings
```

**Drawer .md format** (YAML frontmatter + verbatim body):
```markdown
---
id: 42
source: conversation:session-abc123
room: 7
chunk: 0
tags: ["memory", "field", "topology"]
created: 1712937600000
---

The Aharonov-Bohm effect demonstrates that electromagnetic potentials
can influence physical reality even in regions where fields are zero...
```

- **Drawers**: written to disk IMMEDIATELY on every `file_drawer()` call via atomic write (temp + rename). Verbatim content is never lost — it hits disk before the IPC response returns.
- **Graph**: `index.sexp` stores all nodes and edges as a homoiconic s-expression. Updated on `persist()`.
- **Codebook**: `codebook.json` alongside the palace directory. Also backed to config-store as fallback.
- **Boot restore**: `init()` walks the `palace/` directory tree, parses `.md` frontmatter to reconstruct drawers, loads `index.sexp` for graph, loads `codebook.json`.

> **Note**: Chronicle tables `palace_drawers`, `palace_nodes`, `palace_edges` (V8 schema) are deprecated. Palace data lives in .md files on disk.

### L2b: Concept Memory Store

The semantic structure layer within L2. Maintains an in-RAM concept graph and memory entry store, with Chronicle as the durable backing store. Gives structure to verbatim content -- extracting concepts, tracking co-occurrence, and supporting Hebbian learning through meditation.

### Memory Entries

```lisp
(defstruct memory-entry
  id
  time
  class        ;; :soul | :skill | :daily | :tool
  depth        ;; 0 = raw, 1+ = compressed layers
  content
  tags
  source-ids
  access-count
  last-access)
```

Four entry classes with distinct semantics:

| Class | Purpose | Typical Depth |
|-------|---------|---------------|
| `:soul` | Identity and foundation | 2+ (genesis seeds, near-permanent) |
| `:skill` | Compressed knowledge summaries | 1 (produced by dreaming) |
| `:daily` | Interaction records | 0 (raw, candidates for compression) |
| `:tool` | Tool usage metrics | 0 (raw operational data) |

Depth semantics:

| Depth | Meaning | Behavior |
|-------|---------|----------|
| 0 | Raw | Daily interactions, tool metrics. Candidates for compression. |
| 1 | Compressed | Skill summaries produced by overnight dreaming. |
| 2+ | Genesis seeds | Identity facts, foundational knowledge. Near-permanent. |

### In-Memory State

Five hash tables form the runtime state (protected by `*memory-lock*` mutex):

| Variable | Key | Value | Purpose |
|----------|-----|-------|---------|
| `*memory-store*` | entry ID | memory-entry struct | Primary entry store |
| `*memory-by-class*` | class keyword | list of entry IDs | Class-based index |
| `*memory-concept-nodes*` | concept string | plist (domain, count, entries, classes, depths) | Concept graph nodes |
| `*memory-concept-edges*` | "A\|B" key | plist (a, b, weight, reasons, interdisciplinary) | Concept graph edges |
| `*memory-concept-directed-counts*` | "A>B" key | integer count | Temporal ordering for A-B flux |

### Concept Graph

Concepts are extracted from content by `%index-entry-concepts`:

1. Split content into words, filter stopwords, normalize.
2. Tags are also indexed as concepts -- this creates semantic bridges (e.g., tag `:identity` connects to content words like "who", "name").
3. Each concept gets a node with domain assignment (`%concept-domain` classifies by keyword membership).
4. All concept pairs get symmetric edges (co-occurrence weight incremented).
5. Directed temporal ordering: concepts appearing earlier in text increment the forward count in `*memory-concept-directed-counts*`. This asymmetry feeds the topological flux in Layer 2.

### Meditation (Hebbian Learning)

Post-interaction active learning. Runs when concepts are co-activated during a successful interaction.

Dreaming compresses (offline). Meditation grows (active).

Rules:
- **Strengthen**: existing edges between co-activated concepts have weight boosted by `*meditation-learning-rate*` (default: 2).
- **Bridge**: concept pairs co-activated `*meditation-bridge-threshold*` times (default: 3) without an existing edge get a new bridge edge created.
- **Dampen**: (implicit) edges not reinforced decay over time relative to strengthened ones.

This is the Hebbian rule: concepts that fire together wire together. New bridges emerge between co-activated concepts after sufficient evidence accumulates.

### Compression (Overnight)

Runs during idle-night hours. Non-destructive -- original entries preserved.

Process:
1. Group uncompressed `:daily` entries by intent key (normalized prompt prefix).
2. For each group, compute Kolmogorov ratio and Solomonoff prior:
   - `solomonoff_prior = exp(-new_size / 40)`
   - `occam_pass = kolmogorov_ratio <= 1.1`
3. Groups passing the Occam gate become `:skill` entries at depth 1.
4. Entries scoring >= 0.7 on the signal score are tagged `:crystal` and resist future pruning.
5. Source IDs tracked in `*memory-compressed-source-ids*` to prevent double compression.

### Implementation

| File | Purpose |
|------|---------|
| `src/memory/store/state.lisp` | memory-entry struct, `*memory-store*`, `*memory-by-class*`, `*memory-concept-nodes*`, `*memory-concept-edges*`, `*memory-concept-directed-counts*`, `*memory-lock*` mutex |
| `src/memory/store/concept-map.lisp` | `%index-entry-concepts`, `%upsert-concept-node`, `%upsert-concept-edge`, `memory-map-sexp`, meditation functions |
| `src/memory/store/compression.lisp` | Overnight compression, Solomonoff prior, Occam gate, skill summary building, crystallization |
| `src/memory/store/operations.lisp` | `memory-put`, `memory-get`, `memory-recent`, recall operations |
| `src/memory/store/bootstrap.lisp` | Boot sequence: load from Chronicle, rebuild concept graph, seed DNA, warm-start |
| `src/memory/store.lisp` | Package-level entry point |

### Persistence

- Memory entries: Chronicle `memory_entries` table (deduped by content hash).
- Graph snapshots: Chronicle `graph_snapshots` table.
- Boot: load all entries from Chronicle, rebuild concept graph from entries, seed DNA, warm-start.

## L1: Memory Field (Detail)

The dynamical systems layer. Operates on the concept graph (from L2b) using graph Laplacian field propagation, attractor dynamics, and topological invariants. Recall is relaxation into attractor basins, not keyword search.

State persists as `.sexp` under `nodes/<label>/memory/field/state.sexp`. Reconstructed from L2b's concept graph on boot, then restored from checkpoint.

### Core Mechanism

1. **Graph Laplacian**: `L = D - A` on the weighted concept graph. Sparse CSR representation.
2. **Field solve**: `(L + epsilon*I) * phi = b` via conjugate gradient. Source vector `b` from query concepts.
3. **Spectral decomposition**: first K eigenvectors of L (default K=8). Cached until graph mutation.
4. **Heat kernel**: `K(t) = exp(-t*L)` -- propagation over ALL paths simultaneously. Diffusion time `t` modulated by signal/noise from signalograd.
5. **Attractor dynamics**: three families providing distinct basin geometry.
6. **Topological flux**: Aharonov-Bohm invariants from cycle basis of the concept graph.

### Holographic Scoring

Six signals fused into a single activation score per concept node:

```
activation[i] = 0.25 * field
              + 0.15 * eigenmode
              + 0.20 * heat_kernel
              + 0.20 * basin_affinity
              + 0.10 * topological_flux
              + 0.10 * access
```

All signals are min-max normalized to [0, 1] before fusion. Basin weight ramps from 0.05 to 0.20 during warm-up (first 10 cycles). When the heat kernel is unavailable, the system falls back to legacy weights: 0.40 field + 0.30 eigenmode + 0.20 basin + 0.10 access.

### Three Attractor Families

All use RK4 integration (4th-order Runge-Kutta) and soft saturation `R * tanh(x / R)` for bounded phase space.

#### Thomas Attractor (Domain Routing)

```
dx/dt = sin(y) - b*x
dy/dt = sin(z) - b*y
dz/dt = sin(x) - b*z

b_eff = 0.208 + 0.02*(signal - noise), clamped [0.18, 0.24]
dt = 0.05, state clamped to [-3, 3]
```

Six coexisting basins at b ~ 0.208 (edge of chaos), one per concept-graph domain (music, math, engineering, cognitive, life, generic). Cyclic symmetry `(x -> y -> z -> x)` models biological feedback loops.

Soft basin classification: Boltzmann probability over 6 basins replaces hard octant gate. Each concept node receives a continuous affinity to all basins.

#### Aizawa Attractor (Depth Recall)

```
dx/dt = (z - b)*x - d*y
dy/dt = d*x + (z - b)*y
dz/dt = c + a*z - z^3/3 - (x^2 + y^2)*(1 + e*z) + f*z*x^3

a=0.95, b=0.7, c=0.6, d=3.5, e=0.25, f=0.1, dt=0.01
```

Sphere + tube topology maps to memory depth:
- `|z| > 1.5` classifies as tube (crystallized memories, depth 2+).
- Otherwise classifies as surface (shallow memories, depth 0-1).

#### Halvorsen Attractor (Interdisciplinary Bridging)

```
dx/dt = -a*x - 4*y - 4*z - y^2
dy/dt = -a*y - 4*z - 4*x - z^2
dz/dt = -a*z - 4*x - 4*y - x^2

a = 1.89, dt = 0.01
```

Three-lobed propeller with cyclic symmetry. Each lobe corresponds to a cluster of related domains. Lobes classified by dominant coordinate magnitude. Lyapunov exponents: L1=0.811, L2=0, L3=-4.626. Kaplan-Yorke dimension: 2.175.

### Hysteresis

```
HysteresisTracker {
    current_basin: Basin,
    coercive_energy: f64,      -- accumulated drive toward switch
    threshold: f64,            -- 0.35 + 0.15 * dwell/(dwell + 20)
    dwell_ticks: u64,
}
```

Update rules:
- If `proposed_basin != current_basin`: `coercive_energy += drive_energy`.
- Decay: `coercive_energy *= 0.92` per tick.
- Switch when `coercive_energy > threshold`.
- After switch: reset `coercive_energy = 0`, `dwell_ticks = 0`.
- Longer dwell raises threshold -- established basins resist perturbation.

### Heat Kernel

```
K(t) = exp(-t * L)
```

The heat kernel propagates signal over ALL paths through the graph simultaneously (Feynman path integral analog). Diffusion time t is computed from signal/noise:

- High SNR: small t (precise local recall).
- Low SNR: large t (broad associative recall).
- Range: `t_min = 0.1` to `t_max = 5.0`.
- Formula: `t = t_max - snr * (t_max - t_min)`.

This implements an uncertainty principle analog: precision in concept space trades off with breadth of association.

### Topological Flux (Aharonov-Bohm Layer)

The concept graph has cycles (loops). Each cycle carries "enclosed flux" -- information visible only when traversing the complete loop, invisible to local field measurements.

Computation:
1. **Cycle basis**: spanning tree complement gives `|E| - |V| + 1` independent cycles (first Betti number).
2. **Circulation**: for each cycle, `Phi_c = sum log(w_forward(i,j) / w_reverse(j,i))`.
3. **Node flux**: per-node sum of `|Phi_c|` for all cycles through the node.
4. **Inter-basin flux**: `flux_between_basins` sums `|Phi_c|` for cycles touching both current and proposed basins.

The directed edge weights come from Layer 1's `*memory-concept-directed-counts*` (temporal ordering of concept co-occurrence). This breaks graph symmetry and creates the gauge field.

The flux is a NON-DECAYING component in hysteresis -- structural connectivity persists regardless of temporal decay. It contributes 10% of the holographic score.

### Invariant Measure

Histogram tracking of attractor visits. The system maintains a basin visit histogram that is robust to numerical chaos -- even when individual trajectories diverge, the invariant measure (long-term visit distribution) converges. This provides stable domain routing despite chaotic dynamics.

### Dreaming (Field Self-Maintenance)

Offline field maintenance guided by Landauer's principle: erasing information has entropy cost, so merging (compression) is preferred over deletion (pruning).

Algorithm: Brandes' betweenness centrality + quiescent eigenmode projection score each node. Three outcomes:

| Outcome | Score Threshold | Betweenness | Action |
|---------|----------------|-------------|--------|
| Prune | < 0.02 | ~0 | Node on no shortest paths. K(m\|graph) ~ 0. Safe to delete. |
| Merge | < 0.15 | any | Entries in same basin compressed into one at depth+1. Landauer cost ~ 0. |
| Crystallize | > 0.80 | any | Structural skeleton entries promoted in depth. Resist future decay. |

Entropy delta tracked: `dS = sum(pruned) landauer_cost - sum(crystallized) compression_gain`. Healthy dreaming has `dS <= 0` (net compression). Triggered by heartbeat every 30 ticks.

### Implementation

| File | Purpose |
|------|---------|
| `lib/core/memory-field/src/lib.rs` | FieldState, mod declarations, pub API re-exports |
| `lib/core/memory-field/src/graph.rs` | SparseGraph (CSR), build_graph, concept_index, laplacian_mul, betweenness_centrality (Brandes O(VE)) |
| `lib/core/memory-field/src/field.rs` | Conjugate gradient solver, build_source_vector, solve_field, edge_currents |
| `lib/core/memory-field/src/spectral.rs` | Eigenmode decomposition (inverse iteration + deflation), spectral cache, eigenmode_project, eigenmode_activate, heat_kernel_activate |
| `lib/core/memory-field/src/attractor.rs` | ThomasState, AizawaState, HalvorsenState, step functions (RK4), soft_saturate, BasinClassifier trait |
| `lib/core/memory-field/src/attractor_api.rs` | Public API for attractor stepping |
| `lib/core/memory-field/src/basin.rs` | Basin enum, HysteresisTracker, domain-to-basin mapping, compute_basin_affinity (soft Boltzmann) |
| `lib/core/memory-field/src/scoring.rs` | Activation scoring: holographic 6-signal fusion with configurable warm-up |
| `lib/core/memory-field/src/recall.rs` | compute_recall_pure, field_recall, field_recall_structural, compute_diffusion_time, RecallResult, ConceptActivation |
| `lib/core/memory-field/src/topology.rs` | Cycle basis (BFS spanning tree complement), compute_circulations, compute_node_flux, flux_between_basins |
| `lib/core/memory-field/src/dream.rs` | Dreaming algorithm: Landauer-guided prune/merge/crystallize, entropy tracking |
| `lib/core/memory-field/src/command.rs` | FieldCommand, FieldResult (Service pattern) |
| `lib/core/memory-field/src/config.rs` | Configuration helpers (cfg_f64, cfg_i64) |
| `lib/core/memory-field/src/model.rs` | Data model types |
| `lib/core/memory-field/src/interpret.rs` | Sexp dispatch interpretation |
| `lib/core/memory-field/src/serialize.rs` | State serialization |
| `lib/core/memory-field/src/api.rs` | Public API surface (init, load_graph, recall, step, status, checkpoint) |
| `lib/core/memory-field/src/error.rs` | Error handling, clamp helper |
| `src/ports/memory-field.lisp` | Lisp port -- IPC bridge to the Rust crate |
| `lib/core/memory-field/tests/harmony_tests.rs` | Harmony integration tests |
| `lib/core/memory-field/tests/integration_tests.rs` | Integration tests |

### Persistence

Memory-field state persists to `.sexp` files under `nodes/<label>/memory/field/`:

- **Field checkpoint**: `state.sexp` — attractor coordinates (Thomas, Aizawa, Halvorsen), thomas-b, basin state, coercive energy, dwell ticks, threshold, soft basins, signal/noise, cycle, entropy delta, dream count. Written atomically via tmp+rename for crash safety.
- **Save**: `save-to-disk` command (or `save_to_disk()` method). Idempotent — writing the same state repeatedly is safe.
- **Load**: `load-from-disk` command (or `load_from_disk()` method). Parses the checkpoint sexp and restores attractor coordinates, basins, signal/noise.
- **Graph**: loaded from Lisp concept graph via `load-graph` on first `:observe` tick.
- **Spectral cache**: recomputed from graph on load.
- **Basin state**: also recorded in Chronicle `harmonic_snapshots` table (field_basin, field_checkpoint columns) for operational monitoring.

### IPC Dispatch

Component name: `"memory-field"`

| Op | Input | Output |
|----|-------|--------|
| `init` | none | `(:ok)` |
| `load-graph` | `:nodes (...) :edges (...)` | `(:ok :n N :edges E :spectral-recomputed BOOL)` |
| `field-recall` | `:query-concepts (...) :access-counts (...) :limit N` | `(:ok :activations (...))` |
| `field-dream` | none | `(:ok :pruned (...) :merged (...) :crystallized (...) :stats (:entropy-delta F ...))` |
| `step-attractors` | none | `(:ok :thomas (...) :aizawa (...) :halvorsen (...))` |
| `basin-status` | none | `(:ok :current BASIN :dwell N :coercive-energy F :threshold F)` |
| `eigenmode-status` | none | `(:ok :eigenvalues (...) :spectral-version N)` |
| `status` | none | summary sexp |
| `snapshot` | none | full state sexp |
| `checkpoint` | `:path "..."` | `(:ok :digest N)` |
| `restore` | `:path "..."` | `(:ok :digest N)` |
| `reset` | none | `(:ok)` |

## System Evolution State: Signalograd

Signalograd is NOT a memory layer. It is the system's adaptive kernel -- system evolution state that modulates all memory layers through 119 dynamically learned weights. The weights evolve by Hebbian inference from problem-solving feedback.

### Architecture

```
Observation (31-dim) --> Input matrix --> 32-dim latent space
                                              |
                          Lorenz attractor modulation
                                              |
                          32 Hopfield memory slots (cosine similarity recall)
                                              |
                          5 readout heads --> 22 projection deltas
```

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `LATENT_DIM` | 32 | Latent space dimensionality |
| `INPUT_DIM` | 31 | Observation vector size |
| `MEMORY_SLOTS` | 32 | Hopfield-like memory slots |
| `HEAD_COUNT` | 5 | Readout heads |
| `PHI` | 1.618... | Golden ratio (basis function phase) |
| `DYNAMIC_WEIGHT_COUNT` | 119 | Total learned weights |

### Five Readout Heads

| Head | Purpose | Key Targets |
|------|---------|-------------|
| Harmony | Quality signal (reward, noise, cleanliness) | 8 target weights |
| Routing | Cost/latency/success optimization | 4 target weights |
| Memory | Memory pressure and recall modulation | 8 target weights |
| Evolution | Rewrite readiness and stability | 4 target weights |
| Security | Noise, errors, decorative density | 3 target weights |

### The 119 Dynamic Weights

All weights start from initial conditions (INIT_* constants) and evolve through Hebbian learning. Weight groups:

| Group | Count | Index Range | Purpose |
|-------|-------|-------------|---------|
| Lorenz modulation | 14 | 0..14 | Sigma/rho/beta coupling, dt control, dx/dy/dz coupling |
| Lorenz auxiliary | 4 | 14..18 | Energy normalization, basis phase/scale |
| Hopfield memory | 4 | 18..22 | Strength threshold, similarity base, recall threshold |
| Learning | 4 | 22..26 | Oja regularization, weight decay, learning rate, usage decay |
| Latent | 3 | 26..29 | Mean subtraction, recurrence, recall coupling |
| Confidence | 6 | 29..35 | Stability, anti-chaos, anti-noise, recall, lorenz, cleanliness weights |
| Head targets | 27 | 35..62 | Target weights for all 5 heads |
| Projection scales | 16 | 62..78 | Scale factors for all projection deltas |
| Projection alphas | 11 | 78..89 | Convex mixing coefficients |
| Routing reasoning | 4 | 89..93 | Routing/harmony/recall mixing, scale |
| Routing vitruvian | 2 | 93..95 | Alpha, scale for vitruvian minimum |
| Memory recall limit | 3 | 95..98 | Head scale, strength scale, bound |
| Presentation mixing | 18 | 98..116 | Verbosity, markdown, symbolic, self-ref, decor mixing |
| Network init scales | 3 | 116..119 | Input, recurrent, readout initialization |

### The 3-Body Problem

User x Agent x Model form a dynamical triad. Stability emerges through weight learning:

```
Reward = vitruvian_signal - noise - chaos - errors - queue_depth
```

Acceptance criterion for Hopfield memory storage:

```
accepted = feedback.accepted
        OR (reward > 0.58
            AND stability > 0.55
            AND user_affinity > 0.35
            AND cleanliness > 0.55)
```

When accepted: current latent state is stored into the nearest Hopfield slot (cosine similarity > 0.92) or the weakest slot. Slot content is blended: `0.72 * old + 0.28 * new`.

When rejected: all memory strengths decay by factor 0.999.

### Weight Learning

Dynamic Hebbian update:

```
eta = clamp(0.025 + 0.035*reward + 0.015*stability + 0.010*affinity + 0.005*cleanliness, 0.02, 0.09)
```

Per readout head: `update_local_weights(head, latent, target, eta, dynamic_weights)`.

The confidence error drives the Hebbian update: `dw = eta * confidence_error * signal_correlation`.

### Lorenz Attractor

```
dx = sigma*(y - x) + dw[9]*(signal - noise) + dw[10]*(actor_load - actor_stalls)
dy = x*(rho - z) - y + dw[11]*route_success - dw[12]*cost_pressure
dz = x*y - beta*z + dw[13]*graph_density

sigma = 10.0 + dw[0]*signal - noise
rho = 28.0 + dw[1]*(global_score - chaos_risk) + dw[2]*route_success
beta = 8/3 + dw[3]*memory_pressure
dt = clamp(dw[4] + dw[5]*stability + dw[6]*novelty, dw[7], dw[8])
```

State clamped: x in [-40, 40], y in [-50, 50], z in [0, 60].

### 22 Projection Deltas

The 5 heads produce 22 projection deltas that modulate all other layers:
- Harmony: signal delta, noise delta.
- Rewrite: signal delta, chaos delta.
- Evolution: aggression delta.
- Routing: price delta, speed delta, success delta, reasoning delta, vitruvian-min delta.
- Memory: crystal delta, recall-limit delta.
- Security: dissonance delta, anomaly delta.
- Presentation: verbosity, markdown, symbolic, self-reference, decor deltas.

Each delta follows the convex mixing pattern: `delta = clamp((alpha * primary + (1-alpha) * secondary) * scale, -scale, scale)`.

### Implementation

| File | Purpose |
|------|---------|
| `lib/core/signalograd/src/lib.rs` | Module declarations, pub API |
| `lib/core/signalograd/src/kernel.rs` | Lorenz update, latent normalization, cosine similarity, Hopfield recall, update_local_weights, dot product, lorenz_energy, lorenz_basis |
| `lib/core/signalograd/src/model.rs` | KernelState, LorenzState, Observation, Feedback, Projection; constants (LATENT_DIM, INPUT_DIM, MEMORY_SLOTS, HEAD_COUNT, PHI) |
| `lib/core/signalograd/src/weights.rs` | All 119 INIT_* constants, DYNAMIC_WEIGHT_COUNT, DW_* index layout, initial_dynamic_weights() builder |
| `lib/core/signalograd/src/feedback.rs` | feedback_targets, apply_feedback, remember_state (Hopfield storage), acceptance criterion |
| `lib/core/signalograd/src/observation.rs` | observation_vector, posture_scalar |
| `lib/core/signalograd/src/api.rs` | Public API surface |
| `lib/core/signalograd/src/checkpoint.rs` | State serialization/restore |
| `lib/core/signalograd/src/format.rs` | S-expression output formatting |
| `lib/core/signalograd/src/sexp.rs` | S-expression parsing |
| `lib/core/signalograd/src/error.rs` | Error handling, clamp, seeded_weight |
| `src/ports/signalograd.lisp` | Lisp port -- IPC bridge to the Rust crate |

### Persistence

Full state persisted to `signalograd.sexp`:
- Cycle counter, Lorenz state (x, y, z).
- 32-dim latent vector.
- All matrices (input, recurrent, readout).
- 32 Hopfield memory slots with strengths and usage counts.
- 119 dynamic weights.
- Projection cache.
- Feedback history.

Boot: load from `signalograd.sexp`, resume all dynamics exactly where they left off.

## L3: Datamining (Terraphon)

The datamining layer. Executes platform-specific tools (Lodes) to extract knowledge from external sources. Results that are worth keeping flow into L2 (palace drawers or memory entries). Ephemeral by default — only metadata persists to Chronicle.

### Architecture

| Module | Purpose |
|--------|---------|
| `lib/core/terraphon/src/catalog.rs` | Tool catalog management |
| `lib/core/terraphon/src/lode.rs` | Lode (tool) definition: capabilities, cost, preconditions |
| `lib/core/terraphon/src/planner.rs` | Query planning: select tools, estimate cost |
| `lib/core/terraphon/src/executor.rs` | Tool execution with timeout and error handling |
| `lib/core/terraphon/src/platform.rs` | Platform detection and capability mapping |
| `lib/core/terraphon/src/lib.rs` | Module declarations |
| `lib/core/terraphon/src/sexp.rs` | S-expression parsing for IPC |
| `lib/core/terraphon/src/tools/` | Platform-specific tool implementations |
| `src/ports/terraphon.lisp` | Lisp port -- IPC bridge to the Rust crate |

### Design Principles

- **Ephemeral results**: extracted content is NOT stored in long-term memory. Only metadata (what was queried, when, success/failure) goes to Chronicle.
- **Policy-gated**: all results pass through harmony gates before use. The conductor decides what, if anything, enters the memory palace.
- **Cost-aware**: each Lode has a cost estimate. The planner respects budget constraints.

### Persistence

- Chronicle `terraphon_events` table: metadata only (query, lode_id, timestamp, success, latency).
- No content storage. Results exist only for the duration of the request that triggered them.

## Persistence -- Where Everything Lives

All persistent state lives under `~/.harmoniis/harmonia/` (resolved via config-store `global/state-root`). This directory is the agent's permanent home — it survives binary upgrades, source directory deletion, and uninstallation.

```
~/.harmoniis/harmonia/
|-- nodes/<label>/memory/
|   |-- palace/               <-- L2a: Verbatim palace (.md files)
|   |   |-- index.sexp        <-- Knowledge graph (homoiconic)
|   |   `-- <wing>/
|   |       `-- <room>/
|   |           |-- 000001.md <-- Verbatim drawer (YAML frontmatter + content)
|   |           `-- ...
|   |-- codebook.json         <-- AAAK entity codes
|   `-- field/                <-- L1: Memory field state
|       `-- state.sexp        <-- Attractor coords, basins, signal/noise, cycle
|
|-- chronicle.db              <-- System operational logs (NOT a memory layer)
|   |-- memory_entries        <-- Layer 1: memory content (deduped by content_hash)
|   |-- graph_snapshots       <-- Layer 1/2: concept graph s-expressions
|   |-- graph_nodes           <-- Layer 1/2: relational node decomposition
|   |-- graph_edges           <-- Layer 1/2: relational edge decomposition
|   |-- palace_events         <-- Layer 0: operation audit log (metadata only)
|   |-- harmonic_snapshots    <-- Layer 2: vitruvian triad, field basin state, field_checkpoint sexp
|   |-- signalograd_events    <-- Layer 3: observation/feedback log with checkpoint paths
|   |-- terraphon_events      <-- Datamining: query metadata (no content)
|   |-- delegation_log        <-- Orchestration: model selection decisions
|   |-- memory_events         <-- Compression/crystallization events
|   |-- harmony_trajectory    <-- 5-minute downsampled buckets
|   |-- supervision_specs     <-- Task supervision verdicts
|   |-- phoenix_events        <-- Supervisor lifecycle
|   |-- ouroboros_events      <-- Self-repair events
|   |-- error_events          <-- Error audit trail
|   |-- palace_drawers        <-- DEPRECATED (V8 schema, no longer written)
|   |-- palace_nodes          <-- DEPRECATED (V8 schema, no longer written)
|   `-- palace_edges          <-- DEPRECATED (V8 schema, no longer written)
|
|-- signalograd.sexp          <-- Full kernel state (auto-persisted every :stabilize phase):
|                                  119 dynamic weights, Lorenz (x,y,z), latent[32],
|                                  input_matrix[32x31], recurrent_matrix[32x32],
|                                  readout_weights[5x32], 32 Hopfield memory slots,
|                                  memory_strengths[32], memory_usage[32],
|                                  last_projection, last_feedback, last_observation
|
|-- config.db                 <-- SQLite config-store (scope:key:value)
|   |-- mempalace:codebook    <-- AAAK codebook (JSON: entity-to-code mappings)
|   |-- global:state-root     <-- Path to this directory
|   `-- [component:key]       <-- All other config (paths, feature flags, safety bounds)
|
|-- vault.db                  <-- AES-256-GCM encrypted secrets (API keys, tokens)
|-- metrics.db                <-- Model performance metrics (llm_perf, parallel_tasks, models)
`-- state/                    <-- Runtime transients (PID files, IPC sockets, locks)
```

### Chronicle Tables (Schema V9) -- System Operational Logs Only

Chronicle stores system operational logs and metadata. Primary data persistence for Palace (Layer 0) and Memory Field (Layer 2) now uses files on disk.

| Table | Layer | What's Stored | Key Columns |
|-------|-------|---------------|-------------|
| `palace_events` | L2a | Operation audit log (metadata only) | event_type, operation, node_id, label, detail |
| `memory_entries` | L2b | Memory entries (deduped by content_hash) | id, ts, content, tags, source_ids, access_count, content_hash |
| `graph_snapshots` | L2b | Concept graph state as s-expression | sexp, node_count, edge_count, digest |
| `graph_nodes` | L2b | Relational node decomposition | snapshot_id, concept, domain, count, depth_min/max |
| `graph_edges` | L2b | Relational edge decomposition | snapshot_id, node_a, node_b, weight, interdisciplinary |
| `harmonic_snapshots` | L1 | Vitruvian triad + field state + **field_checkpoint sexp** | signal, noise, field_basin, field_checkpoint |
| `signalograd_events` | Sys | Observation/feedback with checkpoint path | cycle, confidence, checkpoint_path, checkpoint_digest |
| `memory_events` | L2b | Compression/crystallization events | event_type, entries_created, compression_ratio |
| `terraphon_events` | L3 | Query metadata (no content) | lode_id, domain, strategy, elapsed_ms |
| `delegation_log` | Sys | Model delegation decisions | model_chosen, cost_usd, latency_ms, success |
| `palace_drawers` | -- | **DEPRECATED** (V8 schema, no longer written) | -- |
| `palace_nodes` | -- | **DEPRECATED** (V8 schema, no longer written) | -- |
| `palace_edges` | -- | **DEPRECATED** (V8 schema, no longer written) | -- |

Chronicle implementation: `lib/core/chronicle/src/` with per-table modules in `lib/core/chronicle/src/tables/`.

### Auto-Persistence Schedule

| State | When Persisted | Where |
|-------|---------------|-------|
| Signalograd full state | Every `:stabilize` phase (~9 seconds) | `signalograd.sexp` |
| Memory field checkpoint | Every `:stabilize` phase | `nodes/<label>/memory/field/state.sexp` |
| Memory field basin (operational log) | Every `:stabilize` phase | `harmonic_snapshots.field_checkpoint` |
| Memory entries | On every `memory-put` | `memory_entries` |
| Palace drawers | On every `file_drawer()` call (immediate) | `nodes/<label>/memory/palace/<wing>/<room>/*.md` |
| Palace graph | On every `persist()` call | `nodes/<label>/memory/palace/index.sexp` |
| AAAK codebook | On every `persist()` call | `config.db` |
| Concept graph snapshot | Periodic | `graph_snapshots` |
| Basin state | Every `:stabilize` phase | `harmonic_snapshots` (4 columns) |

### Uninstall Safety

The installer has NO uninstall, remove, or purge logic. The `~/.harmoniis/` directory is NEVER deleted by harmonia commands. All memory, weights, and state persist indefinitely. This is by design -- the memory system is the agent's accumulated experience.

## Signal Flow Through Memory Layers

```
User message arrives
  |
  v
L0 (Boot): DNA identity already loaded (immutable foundation)
  |
  v
L2 (Persistent Store):
  |  palace-file-drawer(content) --> .md file written to disk IMMEDIATELY
  |  %index-entry-concepts(content) --> concept graph updated
  |  memory-put(:daily, content) --> Chronicle
  |  memory-meditate(activated-concepts) --> Hebbian edge strengthening
  |
  v
L1 (Field): memory-field-load-graph() --> Laplacian + spectral + topology
  |  memory-field-recall(query) --> heat kernel + soft basins + flux --> scored activations
  |
  v
System State (Signalograd): signalograd-observe(observation) --> weight learning
  |  Reward from problem-solving success --> confidence_error --> Hebbian weight update
  |  22 projection deltas modulate ALL memory layers
  |
  v
L3 (Datamining): terraphon-datamine-for() if needed --> results flow into L2
  |
  v
Response to user (informed by all layers)
```

### Recall Path Detail

When a query arrives:

1. L2a palace context tier selected (L0/L1/L2/L3 based on budget).
2. L2b concept store extracts concepts from the query, finds matching entries.
3. L1 memory field receives the concept signature:
   a. Builds source vector from query concepts.
   b. Solves `(L + epsilon*I) * phi = b` via conjugate gradient.
   c. Projects signal onto eigenmodes (Chladni patterns).
   d. Computes heat kernel propagation at diffusion time t (from signalograd signal/noise).
   e. Evaluates soft basin affinity (Thomas/Aizawa/Halvorsen).
   f. Computes topological flux from cycle basis.
   g. Fuses all 6 signals into holographic activation scores.
   h. Returns top-k activated concepts with entry IDs.
4. Signalograd's projection deltas modulate recall limits, crystal thresholds, and routing.
5. L2b recall path maps entry IDs to full memory-entry structs for context injection.

## Idle-Time Processing

```
Night window (1-5 AM local, idle >= 900s):

  L2b (Concept): compression.lisp
    --> Group daily entries by intent key
    --> Solomonoff prior + Occam gate
    --> Crystallize (score >= 0.7)
    --> Produce :skill summaries at depth 1

  L1 (Field): memory-field-dream()
    --> Brandes betweenness centrality
    --> Eigenmode projection scoring
    --> Prune (score < 0.02, centrality ~ 0)
    --> Merge (score < 0.15, same basin, depth+1)
    --> Crystallize (score > 0.80, promote depth)
    --> Track entropy delta (Landauer's principle)

  L2a (Palace): palace-compress(drawer_ids)
    --> AAAK compression (non-destructive)
    --> Codebook updated and persisted
    --> Original .md drawers preserved on disk
```

## The Aharonov-Bohm Connection

The memory field (Layer 2) implements discrete analogs of the Aharonov-Bohm topological invariants:

1. **Cycles as solenoids**: concept graph cycles carry "enclosed flux" -- information invisible to local field measurements but detectable by traversing the full loop.

2. **Directed counts as gauge field**: Layer 1's `*memory-concept-directed-counts*` (temporal ordering: A appears before B) creates asymmetric edge weights. This breaks graph symmetry and produces the vector potential.

3. **Circulation formula**: `Phi_c = sum_{(i,j) in cycle} log(w_forward(i,j) / w_reverse(j,i))`. For symmetric graphs all circulations are zero. Temporal ordering breaks the symmetry.

4. **Non-decaying invariant**: the topological flux enters hysteresis as a component that does NOT decay exponentially. Structural connectivity persists regardless of the `0.92` per-tick decay of coercive energy.

5. **Heat kernel as path integral**: `K(t) = exp(-t*L)` explores ALL paths through the graph simultaneously. This is the classical analog of the Feynman path integral. Different diffusion times t explore different path scales.

6. **Holographic principle**: effects manifest only where interaction with the field occurs. The 2D graph surface encodes the full vibrational structure -- different queries excite different eigenmodes, giving frequency-selective recall without explicit categorization.

## The Kolmogorov Reduction

The irreducible description of the full memory system:

- **L0**: DNA seeds = immutable identity boundary. Loaded once, never modified.
- **L1**: Graph Laplacian + attractors + heat kernel + topology = dynamical field. Recall is relaxation into attractor basins, not search.
- **L2**: Verbatim .md files + concept graph + Hebbian meditation = persistent searchable store. Content enters exactly. Compression is non-destructive.
- **L3**: Cross-node cross-tool mining = knowledge discovery. Results flow into L2.
- **System state**: 119 learned weights + Lorenz + Hopfield = adaptive kernel. Problem-solving feedback drives continuous weight evolution. NOT a memory layer.

**Common constant**: Hebbian learning rule `dw = eta * error * signal`. Appears in:
- Meditation (Layer 1): edge weight boost from co-activation.
- Field dreaming (Layer 2): eigenmode projection scoring.
- Signalograd (Layer 3): dynamic weight update from confidence error.

**Common structure**: Convex combination `alpha * primary + (1 - alpha) * secondary + epsilon`. Appears in:
- Scoring weights (Layer 2): 6-signal holographic fusion.
- Projection deltas (Layer 3): all 22 deltas follow this pattern.
- Vitruvian triad: user x agent x model mixing.

## Configuration Reference

### Memory-Field Config (scope: `memory-field`)

| Key | Default | Purpose |
|-----|---------|---------|
| `spectral-k` | 8 | Number of eigenvectors to compute |
| `solver-max-iter` | 50 | Conjugate gradient iteration limit |
| `solver-tol` | 0.001 | CG convergence tolerance |
| `solver-epsilon` | 0.01 | Laplacian regularization |
| `activation-threshold` | 0.1 | Minimum activation to include in results |
| `basin-filter-enabled` | true | Whether to filter by current attractor basin |
| `thomas-b-base` | 0.208 | Base Thomas dissipation parameter |
| `hysteresis-threshold-base` | 0.35 | Base coercive threshold for basin switching |
| `hysteresis-decay` | 0.92 | Per-tick decay of coercive energy |
| `heat-kernel-t-min` | 0.1 | Minimum diffusion time (high SNR) |
| `heat-kernel-t-max` | 5.0 | Maximum diffusion time (low SNR) |
| `warm-up-cycles` | 10 | Basin weight ramp-up period |
| `basin-weight-initial` | 0.05 | Basin weight during warm-up start |
| `dream-prune-threshold` | 0.02 | Score below which nodes are pruned |
| `dream-merge-threshold` | 0.15 | Score below which nodes are merged |
| `dream-crystallize-threshold` | 0.80 | Score above which nodes are crystallized |
| `decay-lambda` | 0.01 | Temporal decay rate for access counts |

### Mempalace Config (scope: `mempalace`)

| Key | Default | Purpose |
|-----|---------|---------|
| `l1-max-entries` | 15 | Maximum entries for L1 context tier |
| `l2-max-entries` | 20 | Maximum entries for L2 context tier |
| `l3-max-entries` | 30 | Maximum entries for L3 context tier |
| `codebook` | (JSON) | AAAK codebook state |

## Complete File Inventory

### Layer 0 -- Mempalace (Rust Crate)

| File | Description |
|------|-------------|
| `lib/core/mempalace/src/lib.rs` | Crate root: PalaceState, init, persist, health_check |
| `lib/core/mempalace/src/graph.rs` | Knowledge graph: nodes (Wing/Room/Tunnel), edges, domains |
| `lib/core/mempalace/src/drawer.rs` | Drawer storage: verbatim content with provenance |
| `lib/core/mempalace/src/aaak.rs` | AAAK compression: entity coding, codebook operations |
| `lib/core/mempalace/src/codebook.rs` | LRU codebook (256 entries): JSON serialization |
| `lib/core/mempalace/src/compress.rs` | Drawer-level compression utilities |
| `lib/core/mempalace/src/layers.rs` | Tiered context: L0 (identity), L1 (essential), L2 (filtered), L3 (deep) |
| `lib/core/mempalace/src/query.rs` | Graph traversal and query resolution |
| `lib/core/mempalace/src/sexp.rs` | S-expression parsing for IPC |
| `src/ports/mempalace.lisp` | Lisp-side IPC bridge |
| `lib/core/mempalace/tests/integration.rs` | Integration tests |

### Layer 1 -- Concept Memory Store (Lisp)

| File | Description |
|------|-------------|
| `src/memory/store/state.lisp` | Entry struct, hash tables, lock, stopwords, config helpers |
| `src/memory/store/concept-map.lisp` | Concept extraction, graph indexing, meditation (Hebbian), directed counts |
| `src/memory/store/compression.lisp` | Overnight compression: Solomonoff prior, Occam gate, crystallization |
| `src/memory/store/operations.lisp` | CRUD: memory-put, memory-get, memory-recent, recall |
| `src/memory/store/bootstrap.lisp` | Boot: Chronicle load, graph rebuild, DNA seeding, warm-start |
| `src/memory/store.lisp` | Package entry point |

### Layer 2 -- Memory Field (Rust Crate)

| File | Description |
|------|-------------|
| `lib/core/memory-field/src/lib.rs` | FieldState, module declarations |
| `lib/core/memory-field/src/graph.rs` | Sparse graph (CSR), Laplacian, betweenness centrality |
| `lib/core/memory-field/src/field.rs` | Conjugate gradient field solver |
| `lib/core/memory-field/src/spectral.rs` | Eigendecomposition, heat kernel, Chladni projection |
| `lib/core/memory-field/src/attractor.rs` | Thomas, Aizawa, Halvorsen dynamics (RK4 + soft saturation) |
| `lib/core/memory-field/src/attractor_api.rs` | Public attractor stepping API |
| `lib/core/memory-field/src/basin.rs` | Basin enum, hysteresis tracker, soft Boltzmann classification |
| `lib/core/memory-field/src/scoring.rs` | Holographic 6-signal activation scoring |
| `lib/core/memory-field/src/recall.rs` | Full recall pipeline: field + spectral + heat kernel + basin + topology |
| `lib/core/memory-field/src/topology.rs` | Cycle basis, circulation, node flux, inter-basin flux (A-B invariants) |
| `lib/core/memory-field/src/dream.rs` | Landauer-guided dreaming: prune, merge, crystallize |
| `lib/core/memory-field/src/command.rs` | Service pattern: FieldCommand, FieldResult, FieldDelta |
| `lib/core/memory-field/src/config.rs` | Config-store helpers |
| `lib/core/memory-field/src/model.rs` | Data model types |
| `lib/core/memory-field/src/interpret.rs` | Sexp dispatch interpretation |
| `lib/core/memory-field/src/serialize.rs` | State serialization |
| `lib/core/memory-field/src/api.rs` | Public API surface |
| `lib/core/memory-field/src/error.rs` | Error handling, clamp |
| `src/ports/memory-field.lisp` | Lisp-side IPC bridge |
| `lib/core/memory-field/tests/harmony_tests.rs` | Harmony integration tests |
| `lib/core/memory-field/tests/integration_tests.rs` | Field integration tests |

### Layer 3 -- Signalograd (Rust Crate)

| File | Description |
|------|-------------|
| `lib/core/signalograd/src/lib.rs` | Crate root, module declarations |
| `lib/core/signalograd/src/kernel.rs` | Core: Lorenz, latent, Hopfield recall, cosine similarity, weight update |
| `lib/core/signalograd/src/model.rs` | KernelState, Observation, Feedback, Projection; dimension constants |
| `lib/core/signalograd/src/weights.rs` | 119 INIT_* constants, DW_* layout, initial_dynamic_weights() |
| `lib/core/signalograd/src/feedback.rs` | Acceptance criterion, Hopfield storage, feedback targets |
| `lib/core/signalograd/src/observation.rs` | Observation vector construction, posture scalar |
| `lib/core/signalograd/src/api.rs` | Public API surface |
| `lib/core/signalograd/src/checkpoint.rs` | Full state serialization/restore |
| `lib/core/signalograd/src/format.rs` | S-expression output formatting |
| `lib/core/signalograd/src/sexp.rs` | S-expression parsing |
| `lib/core/signalograd/src/error.rs` | Error handling, clamp, seeded_weight |
| `src/ports/signalograd.lisp` | Lisp-side IPC bridge |

### Datamining -- Terraphon (Rust Crate)

| File | Description |
|------|-------------|
| `lib/core/terraphon/src/lib.rs` | Crate root |
| `lib/core/terraphon/src/catalog.rs` | Tool catalog management |
| `lib/core/terraphon/src/lode.rs` | Lode (tool) definition |
| `lib/core/terraphon/src/planner.rs` | Query planning, cost estimation |
| `lib/core/terraphon/src/executor.rs` | Tool execution |
| `lib/core/terraphon/src/platform.rs` | Platform detection |
| `lib/core/terraphon/src/sexp.rs` | S-expression parsing |
| `lib/core/terraphon/src/tools/` | Platform-specific tool implementations |
| `src/ports/terraphon.lisp` | Lisp-side IPC bridge |

### Persistence -- Chronicle (Rust Crate)

| File | Description |
|------|-------------|
| `lib/core/chronicle/src/lib.rs` | Crate root |
| `lib/core/chronicle/src/db.rs` | SQLite connection management |
| `lib/core/chronicle/src/schema.rs` | Schema definition |
| `lib/core/chronicle/src/migrations.rs` | Schema migrations |
| `lib/core/chronicle/src/query.rs` | Query utilities |
| `lib/core/chronicle/src/gc.rs` | Garbage collection |
| `lib/core/chronicle/src/dashboard.rs` | Dashboard queries |
| `lib/core/chronicle/src/tables/mod.rs` | Table module declarations |
| `lib/core/chronicle/src/tables/memory.rs` | memory_entries table |
| `lib/core/chronicle/src/tables/graph.rs` | graph_snapshots table |
| `lib/core/chronicle/src/tables/harmonic.rs` | harmonic_snapshots table |
| `lib/core/chronicle/src/tables/signalograd.rs` | signalograd_events table |
| `lib/core/chronicle/src/tables/palace.rs` | palace_events table |
| `lib/core/chronicle/src/tables/terraphon.rs` | terraphon_events table |
| `lib/core/chronicle/src/tables/delegation.rs` | delegation_log table |
| `lib/core/chronicle/src/tables/ouroboros.rs` | ouroboros (evolution) table |
| `lib/core/chronicle/src/tables/phoenix.rs` | phoenix (recovery) table |
| `lib/core/chronicle/src/tables/error.rs` | Error handling for table operations |

## See Also

- [memory-field-theory.md](memory-field-theory.md) -- theoretical foundations (attractors, spectral theory, information-theoretic pruning)
- [memory-field-crate.md](memory-field-crate.md) -- Rust crate technical reference (IPC dispatch, data flow, configuration)
- [memory-as-a-field.md](memory-as-a-field.md) -- architecture and design spec
- [signalograd-architecture.md](signalograd-architecture.md) -- chaos-computing kernel design
- [concepts-glossary.md](concepts-glossary.md) -- terminology reference
- [lib-crate-reference.md](lib-crate-reference.md) -- complete crate inventory
- [policy-and-state-reference.md](policy-and-state-reference.md) -- policy and state management
