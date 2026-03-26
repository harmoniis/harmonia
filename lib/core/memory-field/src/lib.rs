/// Memory Field — graph Laplacian field propagation for dynamical memory recall.
///
/// Memory is a field, not a database. Recall is relaxation into attractors,
/// not search through records. Relevance is resonance, not matching.

mod api;
mod attractor;
mod basin;
mod error;
mod field;
mod graph;
mod model;
mod scoring;
mod spectral;

use attractor::{AizawaState, HalvorsenState, ThomasState};
use basin::{Basin, HysteresisTracker};
use graph::SparseGraph;

pub use api::*;

/// Complete field state — all field computation lives here.
/// Reconstructed from concept graph on each boot (stateless persistence).
pub(crate) struct FieldState {
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
}

impl FieldState {
    pub(crate) fn new() -> Self {
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
        }
    }
}
