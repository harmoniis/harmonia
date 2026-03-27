/// Public API surface for the memory field engine.
///
/// All functions take &mut FieldState — the actor owns the state.
/// No globals, no singletons, no C FFI.

use crate::attractor::{update_aizawa, update_halvorsen, update_thomas};
use crate::basin::{assign_node_basins, classify_primary_basin, update_hysteresis};
use crate::error::clamp;
use crate::field::{build_source_vector, solve_field};
use crate::graph::{build_graph, Domain};
use crate::scoring::compute_activation;
use crate::spectral::{eigenmode_activate, eigenmode_project, spectral_decompose};
use crate::FieldState;

// ─── Config-driven parameters ───────────────────────────────────────
// All magic numbers flow from config-store with sensible defaults.
// This keeps tuning policy-driven, not hardcoded.

fn cfg_f64(key: &str, default: f64) -> f64 {
    harmonia_config_store::get_own("memory-field", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

fn cfg_i64(key: &str, default: i64) -> i64 {
    harmonia_config_store::get_own("memory-field", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(default)
}

/// Load a concept graph from parsed node and edge lists.
///
/// Rebuilds the sparse graph and triggers spectral recomputation.
pub fn load_graph(
    s: &mut FieldState,
    nodes: Vec<(String, String, i32, Vec<String>)>,
    edges: Vec<(String, String, f64, bool)>,
) -> Result<String, String> {
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

/// Perform field-based recall for given query concepts.
///
/// Returns scored concept activations as sexp.
pub fn field_recall(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    access_counts: Vec<(String, f64)>,
    limit: usize,
) -> Result<String, String> {
    let n = s.graph.n;
    if n == 0 {
        return Ok("(:ok :activations ())".into());
    }

    // Build source potential vector from query concepts.
    let sources = build_source_vector(&s.graph, &query_concepts);

    // Solve the field: (L + εI)·φ = b (parameters from config).
    let max_iter = cfg_i64("solver-max-iter", 50) as usize;
    let tol = cfg_f64("solver-tol", 0.001);
    let epsilon = cfg_f64("solver-epsilon", 0.01);
    let phi = solve_field(&s.graph, &sources, max_iter, tol, epsilon);

    // Compute eigenmode activation (Chladni projection).
    let eigenmode_activation = if !s.eigenvectors.is_empty() {
        let projections = eigenmode_project(&sources, &s.eigenvectors);
        eigenmode_activate(&projections, &s.eigenvectors, n)
    } else {
        vec![0.0; n]
    };

    // Build per-node access count vector.
    let mut access_vec = vec![0.0; n];
    for (concept, count) in &access_counts {
        if let Some(idx) = crate::graph::concept_index(&s.graph, concept) {
            access_vec[idx] = (*count as f64).min(1.0);
        }
    }

    // Score all nodes.
    let activations = compute_activation(
        &phi,
        &eigenmode_activation,
        s.hysteresis.current_basin,
        &s.node_basins,
        &access_vec,
        n,
        cfg_f64("activation-threshold", 0.1),
        s.cycle,
    );

    // Format top-k results as sexp.
    let top_k = activations.iter().take(limit);
    let mut items = Vec::new();
    for act in top_k {
        let node = &s.graph.nodes[act.node_index];
        let entries_sexp: Vec<String> = node.entry_ids.iter().map(|e| format!("\"{e}\"")).collect();
        items.push(format!(
            "(:concept \"{}\" :score {:.3} :entries ({}))",
            node.concept,
            act.score,
            entries_sexp.join(" "),
        ));
    }

    // Update cycle.
    s.cycle += 1;

    Ok(format!(
        "(:ok :activations ({}))",
        items.join(" "),
    ))
}

/// Step all three attractors by one timestep and update hysteresis.
pub fn step_attractors(s: &mut FieldState, signal: f64, noise: f64) -> Result<String, String> {
    // Thomas b parameter modulated by signal quality (all from config).
    let b_base = cfg_f64("thomas-b-base", 0.208);
    let b_scale = cfg_f64("thomas-b-modulation-scale", 0.02);
    let b_min = cfg_f64("thomas-b-min", 0.18);
    let b_max = cfg_f64("thomas-b-max", 0.24);
    let b_eff = clamp(b_base + b_scale * (signal - noise), b_min, b_max);
    s.thomas_b = b_eff;

    let thomas_dt = cfg_f64("thomas-dt", 0.05);
    let aizawa_dt = cfg_f64("aizawa-dt", 0.01);
    let halvorsen_dt = cfg_f64("halvorsen-dt", 0.01);
    update_thomas(&mut s.thomas, b_eff, thomas_dt);
    update_aizawa(&mut s.aizawa, aizawa_dt);
    update_halvorsen(&mut s.halvorsen, halvorsen_dt);

    // Update basin assignment and hysteresis.
    let proposed = classify_primary_basin(&s.thomas);
    let drive_energy = (signal - noise).abs() * 0.1;
    let _switched = update_hysteresis(&mut s.hysteresis, proposed, drive_energy);

    // Re-assign node basins if we have a graph.
    if s.graph.n > 0 {
        let domains: Vec<Domain> = s.graph.nodes.iter().map(|n| n.domain).collect();
        s.node_basins = assign_node_basins(&domains, &s.thomas, &s.aizawa, &s.halvorsen);
    }

    Ok(format!(
        "(:ok :thomas (:x {:.3} :y {:.3} :z {:.3} :b {:.3}) :aizawa (:x {:.3} :y {:.3} :z {:.3}) :halvorsen (:x {:.3} :y {:.3} :z {:.3}) :basin {})",
        s.thomas.x, s.thomas.y, s.thomas.z, s.thomas_b,
        s.aizawa.x, s.aizawa.y, s.aizawa.z,
        s.halvorsen.x, s.halvorsen.y, s.halvorsen.z,
        s.hysteresis.current_basin.to_sexp(),
    ))
}

/// Return current basin status as sexp.
pub fn basin_status(s: &FieldState) -> Result<String, String> {
    Ok(format!(
        "(:ok :current {} :dwell-ticks {} :coercive-energy {:.3} :threshold {:.3})",
        s.hysteresis.current_basin.to_sexp(),
        s.hysteresis.dwell_ticks,
        s.hysteresis.coercive_energy,
        s.hysteresis.threshold,
    ))
}

/// Return eigenmode status as sexp.
pub fn eigenmode_status(s: &FieldState) -> Result<String, String> {
    let eigenvalues_sexp: Vec<String> = s.eigenvalues.iter().map(|v| format!("{v:.4}")).collect();
    Ok(format!(
        "(:ok :eigenvalues ({}) :spectral-version {} :graph-version {})",
        eigenvalues_sexp.join(" "),
        s.spectral_version,
        s.graph_version,
    ))
}

/// Return summary status as sexp.
pub fn status(s: &FieldState) -> Result<String, String> {
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

/// Restore basin state from Chronicle for warm-start.
pub fn restore_basin(
    s: &mut FieldState,
    basin_str: &str,
    coercive_energy: f64,
    dwell_ticks: u64,
    threshold: f64,
) -> Result<String, String> {
    let basin = crate::basin::Basin::from_sexp(basin_str);
    s.hysteresis = crate::basin::HysteresisTracker::restored(
        basin,
        coercive_energy,
        dwell_ticks,
        threshold,
    );
    Ok(format!(
        "(:ok :restored {} :energy {:.3} :dwell {} :threshold {:.3})",
        s.hysteresis.current_basin.to_sexp(),
        coercive_energy,
        dwell_ticks,
        threshold,
    ))
}

/// Reset field state to initial values.
pub fn reset(s: &mut FieldState) -> Result<String, String> {
    *s = FieldState::new();
    Ok("(:ok)".into())
}
