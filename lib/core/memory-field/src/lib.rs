/// Memory Field -- graph Laplacian field propagation for dynamical memory recall.
///
/// Memory is a field, not a database. Recall is relaxation into attractors,
/// not search through records. Relevance is resonance, not matching.

mod api;
mod attractor;
mod attractor_api;
pub mod basin;
pub mod command;
pub(crate) mod config;
mod dream;
mod error;
pub mod field;
pub mod graph;
mod interpret;
mod model;
mod recall;
pub mod scoring;
mod serialize;
pub mod spectral;

use attractor::{AizawaState, HalvorsenState, ThomasState};
use basin::HysteresisTracker;
use graph::SparseGraph;

// ── Typed API: actor-owned state, no singletons ──────────────────────
pub use api::dream_stats;
pub use api::edge_current_status;
pub use api::{
    basin_status, current_basin, eigenmode_status, field_dream, field_recall,
    field_recall_structural, load_graph, reset, restore_basin, status, step_attractors,
    ConceptActivation, RecallResult,
};
// Dream report type and serializer.
pub use api::{DreamReport, dream_report_to_sexp};
// Phase 7: Cross-node memory digest.
pub use api::{compute_digest, MemoryDigest};
// Phase 8: Genesis improvement.
pub use api::{bootstrap, load_genesis, GenesisEntry};
pub use basin::Basin;
pub use graph::Domain;
// Free Monad types: command, result, delta.
pub use command::{FieldCommand, FieldDelta, FieldResult};

/// Re-export sexp escape from the shared protocol crate.
pub(crate) use harmonia_actor_protocol::sexp_escape as graph_sexp_escape;

/// Complete field state -- all field computation lives here.
/// Reconstructed from concept graph on each boot (stateless persistence).
pub struct FieldState {
    pub(crate) graph: SparseGraph,
    pub(crate) thomas: ThomasState,
    pub(crate) aizawa: AizawaState,
    pub(crate) halvorsen: HalvorsenState,
    pub(crate) hysteresis: HysteresisTracker,
    pub(crate) eigenvalues: Vec<f64>,
    pub(crate) eigenvectors: Vec<Vec<f64>>,
    pub(crate) graph_version: u64,
    pub(crate) spectral_version: u64,
    pub(crate) node_basins: Vec<Basin>,
    pub(crate) cycle: i64,
    pub(crate) thomas_b: f64,
    // ── Entropy bookkeeping (Phase 4D) ──
    pub(crate) cumulative_entropy_delta: f64,
    pub(crate) dream_count: u64,
    pub(crate) total_pruned: u64,
    pub(crate) total_merged: u64,
    pub(crate) total_crystallized: u64,
}

impl FieldState {
    pub fn new() -> Self {
        Self {
            graph: SparseGraph::empty(),
            thomas: ThomasState::default(),
            aizawa: AizawaState::default(),
            halvorsen: HalvorsenState::default(),
            hysteresis: HysteresisTracker::default(),
            eigenvalues: Vec::new(),
            eigenvectors: Vec::new(),
            graph_version: 0,
            spectral_version: 0,
            node_basins: Vec::new(),
            cycle: 0,
            thomas_b: 0.208,
            cumulative_entropy_delta: 0.0,
            dream_count: 0,
            total_pruned: 0,
            total_merged: 0,
            total_crystallized: 0,
        }
    }
}
