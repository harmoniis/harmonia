/// Public API surface for the memory field engine.
///
/// All functions take &mut FieldState -- the actor owns the state.
/// No globals, no singletons, no C FFI.
///
/// Heavy operations (recall, dream, attractor stepping) live in their own modules.
/// This module provides graph loading, status queries, and reset.

use harmonia_actor_protocol::{MemoryError, SexpBuilder};

use crate::basin::assign_node_basins;
use crate::config::cfg_i64;
use crate::graph::{build_graph, Domain};
use crate::spectral::spectral_decompose;
use crate::FieldState;

// Re-export sub-module API functions so callers only need `use api::*`.
pub use crate::attractor_api::{basin_status, restore_basin, step_attractors};
pub use crate::dream::{field_dream, dream_report_to_sexp, DreamReport};
pub use crate::recall::{current_basin, field_recall, field_recall_structural, ConceptActivation, RecallResult};

/// Load a concept graph from parsed node and edge lists.
///
/// Rebuilds the sparse graph and triggers spectral recomputation.
pub fn load_graph(
    s: &mut FieldState,
    nodes: Vec<(String, String, i32, Vec<String>)>,
    edges: Vec<(String, String, f64, bool)>,
) -> Result<String, MemoryError> {
    s.graph = build_graph(&nodes, &edges);
    s.graph_version += 1;

    // Recompute spectral decomposition (k from config).
    let k = (cfg_i64("spectral-k", 8) as usize).min(s.graph.n.saturating_sub(1));
    if k > 0 {
        let (eigenvalues, eigenvectors) = spectral_decompose(&s.graph, k, 200, 1e-6);
        s.eigenvalues = eigenvalues;
        s.eigenvectors = eigenvectors;
    } else {
        s.eigenvalues.clear();
        s.eigenvectors.clear();
    }
    s.spectral_version = s.graph_version;

    // Assign node basins based on current attractor states.
    let domains: Vec<Domain> = s.graph.nodes.iter().map(|n| n.domain).collect();
    s.node_basins = assign_node_basins(&domains, &s.thomas, &s.aizawa, &s.halvorsen);

    Ok(SexpBuilder::ok()
        .key("n").uint(s.graph.n as u64)
        .key("edges").uint((s.graph.col_idx.len() / 2) as u64) // Each undirected edge stored twice in CSR.
        .key("spectral-k").uint(s.eigenvalues.len() as u64)
        .key("graph-version").uint(s.graph_version)
        .build())
}

/// Return eigenmode status as sexp.
pub fn eigenmode_status(s: &FieldState) -> Result<String, MemoryError> {
    let eigenvalues_sexp: Vec<String> = s.eigenvalues.iter().map(|v| format!("{v:.4}")).collect();
    Ok(SexpBuilder::ok()
        .key("eigenvalues").list(&eigenvalues_sexp)
        .key("spectral-version").uint(s.spectral_version)
        .key("graph-version").uint(s.graph_version)
        .build())
}

/// Return summary status as sexp.
pub fn status(s: &FieldState) -> Result<String, MemoryError> {
    Ok(SexpBuilder::ok()
        .key("cycle").int(s.cycle)
        .key("graph-n").uint(s.graph.n as u64)
        .key("graph-version").uint(s.graph_version)
        .key("spectral-k").uint(s.eigenvalues.len() as u64)
        .key("basin").raw(&s.hysteresis.current_basin.to_sexp())
        .key("thomas-b").float(s.thomas_b, 3)
        .build())
}

/// Compute edge current flow from the last solved field.
/// Returns top-K edges by current magnitude as sexp.
pub fn edge_current_status(s: &FieldState) -> Result<String, MemoryError> {
    if s.graph.n == 0 { return Ok("(:ok :currents ())".into()); }
    let uniform: Vec<f64> = vec![1.0 / s.graph.n as f64; s.graph.n];
    let phi = crate::field::solve_field(&s.graph, &uniform, 50, 0.001, 0.01);
    let mut currents = crate::field::edge_currents(&s.graph, &phi);
    currents.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    currents.truncate(10);
    let items: Vec<String> = currents.iter().map(|(a, b, c)| {
        let na = if *a < s.graph.nodes.len() { &s.graph.nodes[*a].concept } else { "?" };
        let nb = if *b < s.graph.nodes.len() { &s.graph.nodes[*b].concept } else { "?" };
        format!("(:a \"{}\" :b \"{}\" :current {:.4})", na, nb, c)
    }).collect();
    Ok(format!("(:ok :currents ({}))", items.join(" ")))
}

/// Return entropy bookkeeping stats as sexp (Phase 4D).
pub fn dream_stats(s: &FieldState) -> Result<String, MemoryError> {
    Ok(SexpBuilder::ok()
        .key("dreams").uint(s.dream_count)
        .key("pruned").uint(s.total_pruned)
        .key("merged").uint(s.total_merged)
        .key("crystallized").uint(s.total_crystallized)
        .key("entropy-delta").float(s.cumulative_entropy_delta, 3)
        .build())
}

/// Reset field state to initial values.
pub fn reset(s: &mut FieldState) -> Result<String, MemoryError> {
    *s = FieldState::new();
    Ok("(:ok)".into())
}

// ── Phase 7: Cross-Node Memory Digest ──────────────────────────────────

/// Compact digest of a node's memory state for gossip protocol.
/// ~500 bytes serialized — small enough to exchange frequently.
#[derive(Clone, Debug)]
pub struct MemoryDigest {
    pub graph_version: u64,
    pub concept_count: usize,
    pub top_concepts: Vec<(String, f64)>,
    pub domain_distribution: [f32; 6],
    pub entropy_estimate: f64,
    pub last_dream_cycle: u64,
}

impl MemoryDigest {
    pub fn to_sexp(&self) -> String {
        let concepts_sexp: Vec<String> = self.top_concepts.iter()
            .map(|(c, s)| format!("(\"{}\" {:.3})", crate::graph_sexp_escape(c), s))
            .collect();
        let domains_sexp = format!("({:.2} {:.2} {:.2} {:.2} {:.2} {:.2})",
            self.domain_distribution[0], self.domain_distribution[1],
            self.domain_distribution[2], self.domain_distribution[3],
            self.domain_distribution[4], self.domain_distribution[5]);
        format!(
            "(:ok :graph-version {} :concepts {} :top ({}) :domains {} :entropy {:.3} :last-dream {})",
            self.graph_version, self.concept_count,
            concepts_sexp.join(" "), domains_sexp,
            self.entropy_estimate, self.last_dream_cycle,
        )
    }
}

/// Compute a compact memory digest for cross-node gossip.
/// Returns structured MemoryDigest; caller serializes at dispatch boundary.
pub fn compute_digest(s: &FieldState) -> Result<MemoryDigest, MemoryError> {
    let n = s.graph.n;

    // Domain distribution: fraction of nodes per domain.
    let mut domain_counts = [0u32; 6];
    for node in &s.graph.nodes {
        let idx = node.domain.index() as usize;
        if idx < 6 { domain_counts[idx] += 1; }
    }
    let total = n.max(1) as f32;
    let domain_distribution: [f32; 6] = [
        domain_counts[0] as f32 / total,
        domain_counts[1] as f32 / total,
        domain_counts[2] as f32 / total,
        domain_counts[3] as f32 / total,
        domain_counts[4] as f32 / total,
        domain_counts[5] as f32 / total,
    ];

    // Top concepts by reference count (degree centrality proxy).
    let mut concept_scores: Vec<(String, f64)> = s.graph.nodes.iter()
        .map(|node| (node.concept.clone(), node.count as f64))
        .collect();
    concept_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    concept_scores.truncate(20);

    let digest = MemoryDigest {
        graph_version: s.graph_version,
        concept_count: n,
        top_concepts: concept_scores,
        domain_distribution,
        entropy_estimate: s.cumulative_entropy_delta,
        last_dream_cycle: s.dream_count,
    };

    Ok(digest)
}

// ── Phase 8: Genesis Improvement ───────────────────────────────────────

/// Structured genesis entry for direct graph seeding.
/// Replaces dense sexp blobs with explicit graph structure.
pub struct GenesisEntry {
    pub concepts: Vec<(String, String)>,  // (concept, domain)
    pub edges: Vec<(String, String, f64)>,  // (from, to, weight)
}

/// Load genesis entries directly into the field graph.
/// Bypasses text-parse-extract pipeline — ~400 bytes instead of ~1500.
pub fn load_genesis(
    s: &mut FieldState,
    entries: Vec<GenesisEntry>,
) -> Result<String, MemoryError> {
    let mut all_nodes: Vec<(String, String, i32, Vec<String>)> = Vec::new();
    let mut all_edges: Vec<(String, String, f64, bool)> = Vec::new();

    for entry in &entries {
        for (concept, domain) in &entry.concepts {
            // Check if concept already exists.
            if !all_nodes.iter().any(|(c, _, _, _)| c == concept) {
                all_nodes.push((concept.clone(), domain.clone(), 1, vec!["genesis".into()]));
            }
        }
        for (from, to, weight) in &entry.edges {
            let interdisciplinary = entries.iter().any(|e| {
                let from_domain = e.concepts.iter().find(|(c, _)| c == from).map(|(_, d)| d.as_str());
                let to_domain = e.concepts.iter().find(|(c, _)| c == to).map(|(_, d)| d.as_str());
                from_domain != to_domain && from_domain.is_some() && to_domain.is_some()
            });
            all_edges.push((from.clone(), to.clone(), *weight, interdisciplinary));
        }
    }

    // Load into graph (reuses existing load_graph path).
    load_graph(s, all_nodes, all_edges)
}

/// Bootstrap sequence: after genesis load, initialize basins and run one dream.
/// Ensures the very first user interaction has correct domain routing.
pub fn bootstrap(s: &mut FieldState) -> Result<String, MemoryError> {
    if s.graph.n == 0 {
        return Err(MemoryError::GraphEmpty);
    }

    // 1. Step attractors once to initialize basin state.
    step_attractors(s, 0.5, 0.1)?;

    // 2. Run one dream cycle to classify foundation nodes.
    let dream_report = field_dream(s)?;

    // 3. Return combined status.
    Ok(format!(
        "(:ok :bootstrapped t :nodes {} :basin {} :dream {})",
        s.graph.n,
        s.hysteresis.current_basin.to_sexp(),
        dream_report_to_sexp(&dream_report),
    ))
}
