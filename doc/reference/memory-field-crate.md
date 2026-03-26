# Memory Field Crate

## Summary

`memory-field` is the graph-native field propagation engine for Harmonia's memory recall subsystem. It operates on the concept graph, computes spectral decompositions, solves discrete Laplacian potentials, and emits activation weights consumed by the Lisp memory recall path.

It is a pure computation kernel — stateless between restarts, reconstructed from Chronicle data, with no direct access to memory entries or conductor policy.

## What Memory-Field Is

- A sparse graph Laplacian solver (conjugate gradient on CSR representation).
- A spectral decomposition cache for concept graph eigenmodes (Chladni patterns).
- An attractor dynamics engine (Thomas, Aizawa, Halvorsen) for basin-aware recall.
- A hysteresis tracker that prevents weak signals from switching memory regimes.
- An activation scoring module combining field potential, eigenmode projection, and basin membership.

## What Memory-Field Is Not

- Not a vector database or embedding store.
- Not a replacement for the 4-class Lisp memory store.
- Not a standalone attractor simulator — Lorenz remains in Signalograd.
- Not allowed to mutate memory entries directly.
- Not a learning system — it computes field dynamics, it does not learn from feedback.

## Core Modules

| Module | Purpose |
|--------|---------|
| `graph.rs` | SparseGraph (CSR), build_graph, concept_index, laplacian_mul |
| `field.rs` | Conjugate gradient solver, build_source_vector, edge_currents |
| `spectral.rs` | Eigenmode decomposition (inverse iteration + deflation), spectral cache |
| `attractor.rs` | Thomas, Aizawa, Halvorsen state structs and update functions |
| `basin.rs` | Basin enum, HysteresisTracker, domain-to-basin mapping, classify functions |
| `scoring.rs` | Activation scoring: 0.40×field + 0.30×eigenmode + 0.20×basin + 0.10×access |
| `api.rs` | Public API surface (init, load_graph, recall, step, status, checkpoint) |
| `sexp.rs` | S-expression parsing (same pattern as signalograd) |
| `format.rs` | S-expression output formatting for field state |
| `checkpoint.rs` | State serialization and restore |
| `error.rs` | Error handling, clamp helper |
| `lib.rs` | FieldState, mod declarations, pub API re-exports |

## Data Flow

```
1. MemoryFieldActor receives concept signature from Gateway
         │
2. Loads current concept graph (nodes + edges from Lisp via IPC)
         │
3. Builds sparse Laplacian L = D - A  (cached, invalidated on graph change)
         │
4. Source vector b constructed from signal's concept nodes
         │
5. Conjugate gradient solve: (L + εI) · φ = b
         │
6. Eigenmode projection: sₖ = ⟨signal, vₖ⟩, a(i) = Σₖ sₖ · vₖ(i)
         │
7. Activation scoring combines field + eigenmode + basin + access
         │
8. Basin filter applied (current attractor basin from hysteresis tracker)
         │
9. Top-k activated concept nodes with entry IDs returned via IPC
         │
10. Lisp recall path maps entry IDs to memory-entry structs
```

## Graph Representation

Compressed Sparse Row (CSR) format for the weighted adjacency matrix:

```
SparseGraph {
    n: usize,                              // node count (typically ~120)
    nodes: Vec<FieldNode>,                 // node data (concept, domain, count, entry_ids)
    row_ptr: Vec<usize>,                   // CSR row pointers (length n+1)
    col_idx: Vec<usize>,                   // CSR column indices
    values: Vec<f64>,                      // CSR edge weights (normalized co-occurrence)
    degree: Vec<f64>,                      // diagonal of degree matrix D
    concept_to_index: Vec<(String, usize)>, // sorted for binary search lookup
}
```

The Laplacian multiply operation `(Lx)_i = degree[i]·x[i] - Σⱼ w_ij·x[j]` traverses the CSR row for each node. Cost: O(|E|) per multiply. For 160 edges, this is negligible.

## Spectral Cache

Eigendecomposition is expensive relative to per-tick budget. Strategy:

- Compute first K eigenvectors on graph mutation (when `graph_version > spectral_version`).
- Cache eigenvalues and eigenvectors until next graph mutation.
- Graph mutations detected via `graph_version` counter, incremented on `load-graph` dispatch.
- Default K = 8 eigenvectors for ~120 nodes captures dominant clustering structure.
- Computation: inverse iteration with deflation, using CG as inner solver.

## Attractor Dynamics

Three attractor families provide distinct basin geometries. All live in memory-field, separate from Signalograd's Lorenz.

### Thomas Attractor

```
dx/dt = sin(y) - b·x
dy/dt = sin(z) - b·y
dz/dt = sin(x) - b·z

b_eff = 0.208 + 0.02·(signal - noise), clamped [0.18, 0.24]
dt = 0.05, state clamped to [-3, 3]
```

Six coexisting basins at b ≈ 0.208, one per concept-graph domain.

### Aizawa Attractor

```
dx/dt = (z-b)·x - d·y
dy/dt = d·x + (z-b)·y
dz/dt = c + a·z - z³/3 - (x²+y²)·(1+e·z) + f·z·x³

a=0.95, b=0.7, c=0.6, d=3.5, e=0.25, f=0.1, dt=0.01
```

Depth classification: |z| > 1.5 → tube (crystal memories), else → surface (shallow).

### Halvorsen Attractor

```
dx/dt = -a·x - 4·y - 4·z - y²
dy/dt = -a·y - 4·z - 4·x - z²
dz/dt = -a·z - 4·x - 4·y - x²

a = 1.89, dt = 0.01
```

Three-fold rotational symmetry for interdisciplinary bridging.

## Hysteresis Model

```
HysteresisTracker {
    current_basin: Basin,
    coercive_energy: f64,      // accumulated drive toward switch
    threshold: f64,            // 0.35 + 0.15 · dwell/(dwell + 20)
    dwell_ticks: u64,
}
```

Update rules:

- If `proposed_basin ≠ current_basin`: `coercive_energy += drive_energy`.
- Decay: `coercive_energy *= 0.92` per tick.
- Switch when `coercive_energy > threshold`.
- After switch: reset `coercive_energy = 0`, `dwell_ticks = 0`.
- Longer dwell → higher threshold → harder to switch (established basins resist perturbation).

## Integration Points

| Boundary | Direction | Protocol |
|----------|-----------|----------|
| Lisp concept graph | receives | Sexp via `load-graph` dispatch (nodes + edges) |
| Lisp recall path | sends | Sexp activation list via `field-recall` dispatch |
| Signalograd | reads | Basin context via IPC (Lorenz state, signal/noise) |
| Chronicle | reads | Graph snapshots for change detection (digest column) |
| Gateway actor | receives | Concept signature via ComponentMsg::Signal |
| Conductor actor | sends | Activated memory IDs as late-binding enrichment |

## IPC Dispatch

Component name: `"memory-field"`

| Op | Input | Output |
|----|-------|--------|
| `init` | none | `(:ok)` |
| `load-graph` | `:nodes (...)  :edges (...)` | `(:ok :n N :edges E :spectral-recomputed BOOL)` |
| `field-recall` | `:query-concepts (...) :access-counts (...) :limit N` | `(:ok :activations (...) :basin (...) :thomas (...))` |
| `step-attractors` | none | `(:ok :thomas (...) :aizawa (...) :halvorsen (...))` |
| `basin-status` | none | `(:ok :current BASIN :dwell N :coercive-energy F :threshold F)` |
| `eigenmode-status` | none | `(:ok :eigenvalues (...) :spectral-version N)` |
| `status` | none | summary sexp |
| `snapshot` | none | full state sexp |
| `checkpoint` | `:path "..."` | `(:ok :digest N)` |
| `restore` | `:path "..."` | `(:ok :digest N)` |
| `reset` | none | `(:ok)` |

## Persistence

Memory-field is stateless between restarts. All state is reconstructed:

- Graph: loaded from Lisp concept graph via `load-graph` on first `:observe` tick.
- Spectral cache: recomputed from graph on load.
- Attractor states: initialized to default positions; converge to basins within a few ticks.
- Hysteresis: reset on restart; basins re-established by incoming signals.

No separate state file. This keeps the system simple and eliminates stale-state bugs.

Optional checkpoint/restore is available for debugging and evolution versioning.

## Configuration

Config-store scope: `memory-field`

| Key | Default | Purpose |
|-----|---------|---------|
| `spectral-k` | 8 | Number of eigenvectors to compute |
| `solver-max-iter` | 50 | Conjugate gradient iteration limit |
| `solver-tol` | 0.001 | CG convergence tolerance |
| `solver-epsilon` | 0.01 | Laplacian regularization (anchors potential to zero-mean) |
| `activation-threshold` | 0.1 | Minimum activation to include in recall results |
| `basin-filter-enabled` | true | Whether to filter by current attractor basin |
| `thomas-b-base` | 0.208 | Base Thomas dissipation parameter |
| `hysteresis-threshold-base` | 0.35 | Base coercive threshold for basin switching |
| `hysteresis-decay` | 0.92 | Per-tick decay of coercive energy |

## Dependencies

```toml
[dependencies]
harmonia-config-store = { version = "0.1.9", path = "../config-store" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

No nalgebra. No ndarray. Pure `f64` arrays and `Vec<f64>`, matching the Signalograd convention. All matrix operations are hand-implemented for the specific sparse structures used.

## See Also

- [memory-as-a-field.md](memory-as-a-field.md) — architecture and design spec
- [memory-field-theory.md](memory-field-theory.md) — theoretical foundations
- [signalograd-architecture.md](signalograd-architecture.md) — existing chaos kernel (attractor state source)
- [lib-crate-reference.md](lib-crate-reference.md) — crate inventory
