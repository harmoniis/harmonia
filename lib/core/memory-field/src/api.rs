/// Public API surface for the memory field engine.
///
/// All functions take &mut FieldState -- the actor owns the state.
/// No globals, no singletons, no C FFI.
///
/// Heavy operations (recall, dream, attractor stepping) live in their own modules.
/// This module provides graph loading, status queries, and reset.

use harmonia_actor_protocol::MemoryError;

use crate::basin::assign_node_basins;
use crate::config::cfg_i64;
use crate::graph::{build_graph, Domain};
use crate::spectral::spectral_decompose;
use crate::FieldState;

// Re-export sub-module API functions so callers only need `use api::*`.
pub use crate::attractor_api::{basin_status, restore_basin, step_attractors};
pub use crate::dream::{field_dream, dream_report_to_sexp, DreamReport};
pub use crate::recall::{current_basin, field_recall, field_recall_structural, ConceptActivation, RecallResult};

/// Pure graph load computation — builds graph, spectral decomposition, and basin assignment.
/// Returns (graph, eigenvalues, eigenvectors, node_basins).
pub(crate) fn compute_load_graph(
    nodes: &[(String, String, i32, Vec<String>)],
    edges: &[(String, String, f64, bool)],
    thomas: &crate::attractor::ThomasState,
    aizawa: &crate::attractor::AizawaState,
    halvorsen: &crate::attractor::HalvorsenState,
) -> (crate::graph::SparseGraph, Vec<f64>, Vec<Vec<f64>>, Vec<crate::basin::Basin>) {
    let graph = build_graph(nodes, edges);

    // Recompute spectral decomposition (k from config).
    let k = (cfg_i64("spectral-k", 8) as usize).min(graph.n.saturating_sub(1));
    let (eigenvalues, eigenvectors) = if k > 0 {
        spectral_decompose(&graph, k, 200, 1e-6)
    } else {
        (Vec::new(), Vec::new())
    };

    // Assign node basins based on current attractor states.
    let domains: Vec<Domain> = graph.nodes.iter().map(|n| n.domain).collect();
    let node_basins = assign_node_basins(&domains, thomas, aizawa, halvorsen);

    (graph, eigenvalues, eigenvectors, node_basins)
}

/// Load a concept graph from parsed node and edge lists.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn load_graph(
    s: &mut FieldState,
    nodes: Vec<(String, String, i32, Vec<String>)>,
    edges: Vec<(String, String, f64, bool)>,
) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::LoadGraph { nodes, edges };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    Ok(result.to_sexp())
}

/// Return eigenmode status as sexp.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn eigenmode_status(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::EigenmodeStatus;
    let (_delta, result) = s.handle(cmd)?;
    Ok(result.to_sexp())
}

/// Return summary status as sexp.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn status(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::Status;
    let (_delta, result) = s.handle(cmd)?;
    Ok(result.to_sexp())
}

/// Pure edge current computation — returns top-K edges as (concept_a, concept_b, current).
pub(crate) fn compute_edge_currents_pure(s: &FieldState) -> Vec<(String, String, f64)> {
    if s.graph.n == 0 { return Vec::new(); }
    let uniform: Vec<f64> = vec![1.0 / s.graph.n as f64; s.graph.n];
    let phi = crate::field::solve_field(&s.graph, &uniform, 50, 0.001, 0.01);
    let mut currents = crate::field::edge_currents(&s.graph, &phi);
    currents.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    currents.truncate(10);
    currents.iter().map(|(a, b, c)| {
        let na = if *a < s.graph.nodes.len() { s.graph.nodes[*a].concept.clone() } else { "?".into() };
        let nb = if *b < s.graph.nodes.len() { s.graph.nodes[*b].concept.clone() } else { "?".into() };
        (na, nb, *c)
    }).collect()
}

/// Compute edge current flow from the last solved field.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn edge_current_status(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::EdgeCurrents;
    let (_delta, result) = s.handle(cmd)?;
    // delta is FieldDelta::None, no need to apply.
    Ok(result.to_sexp())
}

/// Return entropy bookkeeping stats as sexp (Phase 4D).
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn dream_stats(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::DreamStats;
    let (_delta, result) = s.handle(cmd)?;
    Ok(result.to_sexp())
}

/// Reset field state to initial values.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn reset(s: &mut FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::Reset;
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    Ok(result.to_sexp())
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

/// Pure digest computation — returns MemoryDigest without mutation.
pub(crate) fn compute_digest_pure(s: &FieldState) -> MemoryDigest {
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

    MemoryDigest {
        graph_version: s.graph_version,
        concept_count: n,
        top_concepts: concept_scores,
        domain_distribution,
        entropy_estimate: s.cumulative_entropy_delta,
        last_dream_cycle: s.dream_count,
    }
}

/// Compute a compact memory digest for cross-node gossip.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn compute_digest(s: &FieldState) -> Result<MemoryDigest, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::{FieldCommand, FieldResult};
    let cmd = FieldCommand::Digest;
    let (_delta, result) = s.handle(cmd)?;
    match result {
        FieldResult::Digest(d) => Ok(d),
        _ => unreachable!(),
    }
}

// ── Phase 8: Genesis Improvement ───────────────────────────────────────

/// Structured genesis entry for direct graph seeding.
/// Replaces dense sexp blobs with explicit graph structure.
pub struct GenesisEntry {
    pub concepts: Vec<(String, String)>,  // (concept, domain)
    pub edges: Vec<(String, String, f64)>,  // (from, to, weight)
}

/// Pure genesis flattening — converts genesis entries to node/edge lists.
pub(crate) fn flatten_genesis_entries(
    entries: &[GenesisEntry],
) -> (Vec<(String, String, i32, Vec<String>)>, Vec<(String, String, f64, bool)>) {
    let mut all_nodes: Vec<(String, String, i32, Vec<String>)> = Vec::new();
    let mut all_edges: Vec<(String, String, f64, bool)> = Vec::new();

    for entry in entries {
        for (concept, domain) in &entry.concepts {
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

    (all_nodes, all_edges)
}

/// Load genesis entries directly into the field graph.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn load_genesis(
    s: &mut FieldState,
    entries: Vec<GenesisEntry>,
) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::LoadGenesis { entries };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    Ok(result.to_sexp())
}

/// Bootstrap sequence: after genesis load, initialize basins and run one dream.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn bootstrap(s: &mut FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::Bootstrap;
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    Ok(result.to_sexp())
}
