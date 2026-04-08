/// Field-based recall — solve the Laplacian field for query concepts.

use crate::config::{cfg_f64, cfg_i64};
use crate::field::{build_source_vector, solve_field};
use crate::scoring::compute_activation;
use crate::spectral::{eigenmode_activate, eigenmode_project};
use crate::FieldState;

/// Perform field-based recall for given query concepts.
///
/// Returns scored concept activations as sexp.
pub fn field_recall(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    access_counts: Vec<(String, f64, f64)>,
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

    // Build per-node access count vector with depth-aware temporal decay.
    //
    // Philosophy: you don't forget that Kipling's "If" shaped your character.
    // You forget verbatim words but remember meaning. Important memories (high
    // depth, high centrality) resist decay. Noise (depth-0, low centrality) fades.
    //
    // access_decayed = count * exp(-lambda * age_hours / protection)
    // protection = 1 + node.count/10 (more connections -> more structural -> slower decay)
    let decay_lambda = cfg_f64("decay-lambda", 0.01);
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let mut access_vec = vec![0.0; n];
    for (concept, count, last_access) in &access_counts {
        if let Some(idx) = crate::graph::concept_index(&s.graph, concept) {
            let age_hours = if *last_access > 0.0 {
                ((now_unix - last_access) / 3600.0).max(0.0)
            } else {
                0.0 // No last-access info -> no decay (treat as fresh)
            };
            // Protection factor: structural nodes decay slower.
            let node_count = s.graph.nodes[idx].count as f64;
            let protection = 1.0 + node_count / 10.0;
            let decayed = (*count as f64).min(1.0) * (-decay_lambda * age_hours / protection).exp();
            access_vec[idx] = decayed;
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
