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
pub use crate::dream::field_dream;
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

    Ok(format!(
        "(:ok :n {} :edges {} :spectral-k {} :graph-version {})",
        s.graph.n,
        s.graph.col_idx.len() / 2, // Each undirected edge stored twice in CSR.
        s.eigenvalues.len(),
        s.graph_version,
    ))
}

/// Return eigenmode status as sexp.
pub fn eigenmode_status(s: &FieldState) -> Result<String, MemoryError> {
    let eigenvalues_sexp: Vec<String> = s.eigenvalues.iter().map(|v| format!("{v:.4}")).collect();
    Ok(format!(
        "(:ok :eigenvalues ({}) :spectral-version {} :graph-version {})",
        eigenvalues_sexp.join(" "),
        s.spectral_version,
        s.graph_version,
    ))
}

/// Return summary status as sexp.
pub fn status(s: &FieldState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :cycle {} :graph-n {} :graph-version {} :spectral-k {} :basin {} :thomas-b {:.3})",
        s.cycle,
        s.graph.n,
        s.graph_version,
        s.eigenvalues.len(),
        s.hysteresis.current_basin.to_sexp(),
        s.thomas_b,
    ))
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

/// Reset field state to initial values.
pub fn reset(s: &mut FieldState) -> Result<String, MemoryError> {
    *s = FieldState::new();
    Ok("(:ok)".into())
}
