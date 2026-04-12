//! Free Monad command/result/delta types for the memory palace.
//!
//! Commands are pure data. Results are structured.
//! Deltas include disk I/O descriptors -- the apply() function executes them.

use crate::drawer::{Drawer, DrawerSource};
use crate::graph::{NodeKind, Domain, EdgeKind, GraphNode, GraphEdge};
use crate::codebook::AaakCodebook;

/// Commands -- pure data describing what to do.
pub enum PalaceCommand {
    /// File a new drawer (verbatim content).
    FileDrawer {
        content: String,
        room_id: u32,
        source: DrawerSource,
        tags: Vec<String>,
    },
    /// Retrieve a drawer by ID.
    GetDrawer { id: u64 },
    /// Search drawers by query.
    Search {
        query: String,
        room_filter: Option<u32>,
        limit: usize,
    },
    /// Add a node to the knowledge graph.
    AddNode {
        kind: NodeKind,
        label: String,
        domain: Domain,
    },
    /// Add an edge to the knowledge graph.
    AddEdge {
        source: u32,
        target: u32,
        kind: EdgeKind,
        weight: f64,
    },
    /// Persist all state to disk.
    Persist,
    /// Initialize / restore state from disk.
    Init,
}

/// Results -- structured computation output.
pub enum PalaceResult {
    /// Drawer filed successfully.
    DrawerFiled { id: u64, room_id: u32, size: usize },
    /// Drawer retrieved (sexp-formatted content).
    DrawerRetrieved(String),
    /// Search results (sexp-formatted).
    SearchResults(String),
    /// Node added to the graph.
    NodeAdded { id: u32, label: String },
    /// Edge added to the graph.
    EdgeAdded { source: u32, target: u32 },
    /// State persisted to disk.
    Persisted {
        drawers: usize,
        nodes: usize,
        edges: usize,
    },
    /// State initialized from disk.
    Initialized {
        nodes: usize,
        drawers: usize,
        codebook: usize,
    },
}

/// Deltas -- explicit state transitions + disk I/O descriptors.
pub enum PalaceDelta {
    /// No state change.
    None,
    /// New drawer added to store + write .md to disk.
    DrawerAdded {
        drawer: Drawer,
        /// Disk write descriptor: (path, content). Executed in apply().
        disk_write: Option<(std::path::PathBuf, String)>,
    },
    /// Node added to graph.
    NodeAdded { node: GraphNode },
    /// Edge added to graph.
    EdgeAdded { edge: GraphEdge },
    /// Full persistence flush.
    Persisted {
        /// Graph index sexp to write.
        graph_sexp: String,
        graph_path: std::path::PathBuf,
        /// Codebook JSON to write.
        codebook_json: String,
        codebook_path: std::path::PathBuf,
        /// Individual drawer .md writes: (path, content).
        drawer_writes: Vec<(std::path::PathBuf, String)>,
    },
    /// Bulk restore from disk (init).
    Restored {
        drawers: Vec<Drawer>,
        nodes: Vec<GraphNode>,
        edges: Vec<GraphEdge>,
        codebook: AaakCodebook,
        next_drawer_id: u64,
    },
}
