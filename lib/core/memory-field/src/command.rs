//! Free Monad command/result/delta types for the memory field.
//!
//! Commands are pure data — they describe operations without executing them.
//! Results are structured — serialization happens at the dispatch boundary.
//! Deltas are explicit state transitions — applied atomically by the actor.

use crate::basin::Basin;
use crate::graph::SparseGraph;
use crate::attractor::{ThomasState, AizawaState, HalvorsenState};
use crate::basin::HysteresisTracker;
use crate::recall::RecallResult;
use crate::dream::DreamReport;
use crate::api::MemoryDigest;

/// Commands — pure data describing what to do (Free Monad).
pub enum FieldCommand {
    LoadGraph {
        nodes: Vec<(String, String, i32, Vec<String>)>,
        edges: Vec<(String, String, f64, bool)>,
    },
    Recall {
        query_concepts: Vec<String>,
        access_counts: Vec<(String, f64, f64)>,
        limit: usize,
    },
    RecallStructural {
        query_concepts: Vec<String>,
        limit: usize,
    },
    StepAttractors {
        signal: f64,
        noise: f64,
    },
    Dream,
    Bootstrap,
    Digest,
    Status,
    BasinStatus,
    EigenmodeStatus,
    EdgeCurrents,
    DreamStats,
    CurrentBasin,
    RestoreBasin {
        basin_str: String,
        coercive_energy: f64,
        dwell_ticks: u64,
        threshold: f64,
    },
    LoadGenesis {
        entries: Vec<crate::api::GenesisEntry>,
    },
    Reset,
}

/// Results — structured data, serialized only at the dispatch boundary.
pub enum FieldResult {
    GraphLoaded { n: usize, edges: usize, spectral_k: usize, graph_version: u64 },
    Recalled(RecallResult),
    Stepped(SteppedResult),
    Dreamed(DreamReport),
    Bootstrapped(BootstrapResult),
    Digest(MemoryDigest),
    Status(StatusResult),
    BasinStatus(BasinStatusResult),
    EigenmodeStatus(EigenmodeResult),
    EdgeCurrents(Vec<(String, String, f64)>),
    DreamStats(DreamStatsResult),
    CurrentBasin { basin: String, cycle: i64 },
    BasinRestored(BasinRestoredResult),
    GenesisLoaded { n: usize, edges: usize, spectral_k: usize, graph_version: u64 },
    Reset,
}

// Sub-result structs (keep them small and focused)
pub struct SteppedResult {
    pub thomas: (f64, f64, f64),
    pub thomas_b: f64,
    pub aizawa: (f64, f64, f64),
    pub halvorsen: (f64, f64, f64),
    pub basin: String,
}

pub struct BootstrapResult {
    pub nodes: usize,
    pub basin: String,
    pub dream: DreamReport,
}

pub struct StatusResult {
    pub cycle: i64,
    pub graph_n: usize,
    pub graph_version: u64,
    pub spectral_k: usize,
    pub basin: String,
    pub thomas_b: f64,
}

pub struct BasinStatusResult {
    pub current: String,
    pub dwell_ticks: u64,
    pub coercive_energy: f64,
    pub threshold: f64,
}

pub struct EigenmodeResult {
    pub eigenvalues: Vec<f64>,
    pub spectral_version: u64,
    pub graph_version: u64,
}

pub struct DreamStatsResult {
    pub dreams: u64,
    pub pruned: u64,
    pub merged: u64,
    pub crystallized: u64,
    pub entropy_delta: f64,
}

pub struct BasinRestoredResult {
    pub basin: String,
    pub energy: f64,
    pub dwell: u64,
    pub threshold: f64,
}

/// Deltas — explicit state transitions. Applied atomically.
///
/// While this is pub (required by the Service trait), the inner types
/// (SparseGraph, ThomasState, etc.) are pub(crate), so external crates
/// cannot construct or destructure delta variants — only pass them
/// between handle() and apply().
#[allow(private_interfaces)]
pub enum FieldDelta {
    /// No state change (read-only operations).
    None,
    /// Full graph rebuild (load_graph, load_genesis).
    GraphRebuilt {
        graph: SparseGraph,
        eigenvalues: Vec<f64>,
        eigenvectors: Vec<Vec<f64>>,
        graph_version: u64,
        spectral_version: u64,
        node_basins: Vec<Basin>,
    },
    /// Attractor step.
    AttractorStepped {
        thomas: ThomasState,
        thomas_b: f64,
        aizawa: AizawaState,
        halvorsen: HalvorsenState,
        hysteresis: HysteresisTracker,
        node_basins: Vec<Basin>,
    },
    /// Dream completed.
    DreamCompleted {
        entropy_delta: f64,
        pruned_count: u64,
        merged_count: u64,
        crystallized_count: u64,
    },
    /// Recall increments cycle counter.
    CycleIncremented { new_cycle: i64 },
    /// Basin restored from Chronicle.
    BasinRestored { hysteresis: HysteresisTracker },
    /// Full reset.
    Reset,
}
