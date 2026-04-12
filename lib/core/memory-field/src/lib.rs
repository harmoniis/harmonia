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
pub(crate) mod topology;

use attractor::{AizawaState, HalvorsenState, InvariantMeasure, ThomasState};
use basin::HysteresisTracker;
use graph::SparseGraph;
use topology::TopologyState;

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

/// Resolve memory-field state directory: `<memory-root>/field/`.
fn field_state_dir() -> Option<std::path::PathBuf> {
    harmonia_config_store::get_own("node", "memory-root")
        .ok()
        .flatten()
        .map(|root| std::path::PathBuf::from(root).join("field"))
}

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
    // ── Invariant measure + soft basin classification ──
    pub(crate) thomas_measure: InvariantMeasure,
    pub(crate) thomas_soft_basins: [f64; 6],
    // ── Entropy bookkeeping (Phase 4D) ──
    pub(crate) cumulative_entropy_delta: f64,
    pub(crate) dream_count: u64,
    pub(crate) total_pruned: u64,
    pub(crate) total_merged: u64,
    pub(crate) total_crystallized: u64,
    pub(crate) topology: TopologyState,
    // ── Signal/noise state (holographic boundary → bulk coupling) ──
    pub(crate) last_signal: f64,
    pub(crate) last_noise: f64,
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
            thomas_measure: InvariantMeasure::default(),
            thomas_soft_basins: [1.0 / 6.0; 6],
            cumulative_entropy_delta: 0.0,
            dream_count: 0,
            total_pruned: 0,
            total_merged: 0,
            total_crystallized: 0,
            topology: TopologyState::default(),
            last_signal: 0.5,
            last_noise: 0.2,
        }
    }

    /// Serialize field state for Chronicle persistence.
    /// Returns an s-expression string with all recoverable state.
    pub fn checkpoint_sexp(&self) -> String {
        format!(
            "(:thomas ({:.6} {:.6} {:.6}) :thomas-b {:.6} \
             :aizawa ({:.6} {:.6} {:.6}) :halvorsen ({:.6} {:.6} {:.6}) \
             :basin {} :coercive-energy {:.4} :dwell-ticks {} :threshold {:.4} \
             :soft-basins ({:.4} {:.4} {:.4} {:.4} {:.4} {:.4}) \
             :last-signal {:.4} :last-noise {:.4} \
             :cycle {} :graph-version {} :spectral-version {} \
             :topology-cycles {} \
             :entropy-delta {:.6} :dream-count {} \
             :measure-visits {})",
            self.thomas.x, self.thomas.y, self.thomas.z,
            self.thomas_b,
            self.aizawa.x, self.aizawa.y, self.aizawa.z,
            self.halvorsen.x, self.halvorsen.y, self.halvorsen.z,
            self.hysteresis.current_basin.to_sexp(),
            self.hysteresis.coercive_energy,
            self.hysteresis.dwell_ticks,
            self.hysteresis.threshold,
            self.thomas_soft_basins[0], self.thomas_soft_basins[1],
            self.thomas_soft_basins[2], self.thomas_soft_basins[3],
            self.thomas_soft_basins[4], self.thomas_soft_basins[5],
            self.last_signal, self.last_noise,
            self.cycle, self.graph_version, self.spectral_version,
            self.topology.cycles.len(),
            self.cumulative_entropy_delta, self.dream_count,
            self.thomas_measure.total_visits,
        )
    }

    /// Restore attractor coordinates from persisted values.
    pub fn restore_attractors(&mut self, thomas: (f64, f64, f64), aizawa: (f64, f64, f64), halvorsen: (f64, f64, f64)) {
        self.thomas.x = thomas.0;
        self.thomas.y = thomas.1;
        self.thomas.z = thomas.2;
        self.aizawa.x = aizawa.0;
        self.aizawa.y = aizawa.1;
        self.aizawa.z = aizawa.2;
        self.halvorsen.x = halvorsen.0;
        self.halvorsen.y = halvorsen.1;
        self.halvorsen.z = halvorsen.2;
    }

    /// Restore signal/noise state.
    pub fn restore_signal_noise(&mut self, signal: f64, noise: f64) {
        self.last_signal = signal;
        self.last_noise = noise;
    }

    /// Restore soft basin probabilities.
    pub fn restore_soft_basins(&mut self, basins: [f64; 6]) {
        self.thomas_soft_basins = basins;
    }

    /// Save field state to disk as .sexp (atomic write via tmp+rename).
    /// Idempotent: writing the same state repeatedly is safe.
    pub fn save_to_disk(&self) -> Result<(), String> {
        let dir = field_state_dir().ok_or("no memory-root configured")?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join("state.sexp");
        let tmp = dir.join("state.sexp.tmp");
        let sexp = self.checkpoint_sexp();
        std::fs::write(&tmp, &sexp).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load field state from disk .sexp file.
    /// Returns Ok(true) if state was restored, Ok(false) if no file found.
    pub fn load_from_disk(&mut self) -> Result<bool, String> {
        let dir = field_state_dir().ok_or("no memory-root configured")?;
        let path = dir.join("state.sexp");
        if !path.exists() { return Ok(false); }
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        self.restore_from_checkpoint_sexp(&content)?;
        Ok(true)
    }

    /// Parse a checkpoint sexp and restore state.
    fn restore_from_checkpoint_sexp(&mut self, sexp: &str) -> Result<(), String> {
        fn extract_f64(s: &str, key: &str) -> Option<f64> {
            let pos = s.find(key)? + key.len();
            let rest = s[pos..].trim_start();
            let num: String = rest.chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                .collect();
            num.parse().ok()
        }

        fn extract_triple(s: &str, key: &str) -> Option<(f64, f64, f64)> {
            let pos = s.find(key)? + key.len();
            let rest = s[pos..].trim_start();
            let paren_start = rest.find('(')? + 1;
            let paren_end = rest.find(')')?;
            let inner = &rest[paren_start..paren_end];
            let nums: Vec<f64> = inner.split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            if nums.len() >= 3 { Some((nums[0], nums[1], nums[2])) } else { None }
        }

        if let Some((x, y, z)) = extract_triple(sexp, ":thomas") {
            self.thomas.x = x; self.thomas.y = y; self.thomas.z = z;
        }
        if let Some((x, y, z)) = extract_triple(sexp, ":aizawa") {
            self.aizawa.x = x; self.aizawa.y = y; self.aizawa.z = z;
        }
        if let Some((x, y, z)) = extract_triple(sexp, ":halvorsen") {
            self.halvorsen.x = x; self.halvorsen.y = y; self.halvorsen.z = z;
        }
        if let Some(v) = extract_f64(sexp, ":thomas-b ") { self.thomas_b = v; }
        if let Some(v) = extract_f64(sexp, ":last-signal ") { self.last_signal = v; }
        if let Some(v) = extract_f64(sexp, ":last-noise ") { self.last_noise = v; }
        if let Some(v) = extract_f64(sexp, ":coercive-energy ") { self.hysteresis.coercive_energy = v; }
        if let Some(v) = extract_f64(sexp, ":threshold ") { self.hysteresis.threshold = v; }
        if let Some(v) = extract_f64(sexp, ":cycle ") { self.cycle = v as i64; }
        if let Some(v) = extract_f64(sexp, ":entropy-delta ") { self.cumulative_entropy_delta = v; }
        if let Some(v) = extract_f64(sexp, ":dream-count ") { self.dream_count = v as u64; }

        // Parse :soft-basins (p0 p1 p2 p3 p4 p5)
        if let Some(pos) = sexp.find(":soft-basins") {
            let rest = &sexp[pos..];
            if let Some(paren_start) = rest.find('(') {
                if let Some(paren_end) = rest[paren_start..].find(')') {
                    let inner = &rest[paren_start + 1..paren_start + paren_end];
                    let vals: Vec<f64> = inner.split_whitespace()
                        .filter_map(|n| n.parse().ok())
                        .collect();
                    if vals.len() >= 6 {
                        self.thomas_soft_basins = [vals[0], vals[1], vals[2], vals[3], vals[4], vals[5]];
                    }
                }
            }
        }

        Ok(())
    }
}
