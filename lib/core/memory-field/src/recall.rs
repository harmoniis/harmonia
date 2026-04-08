/// Field-based recall — solve the Laplacian field for query concepts.

use harmonia_actor_protocol::MemoryError;

use crate::basin::Basin;
use crate::config::{cfg_f64, cfg_i64};
use crate::field::{build_source_vector, solve_field};
use crate::graph::Domain;
use crate::scoring::compute_activation;
use crate::spectral::{eigenmode_activate, eigenmode_project};
use crate::FieldState;

// ── Structured result types ──

/// A single concept activation from field recall.
#[derive(Clone, Debug)]
pub struct ConceptActivation {
    pub concept: String,
    pub score: f64,
    pub basin: Basin,
    pub domain: Domain,
    pub entry_ids: Vec<String>,
}

/// Structured recall result — computation output before serialization.
#[derive(Clone, Debug)]
pub struct RecallResult {
    pub activations: Vec<ConceptActivation>,
}

impl RecallResult {
    /// Serialize to sexp for IPC. Separate from computation.
    pub fn to_sexp(&self) -> String {
        let items: Vec<String> = self.activations.iter().map(|act| {
            let entries_sexp: Vec<String> = act.entry_ids.iter()
                .map(|e| format!("\"{}\"", crate::graph_sexp_escape(e)))
                .collect();
            format!(
                "(:concept \"{}\" :score {:.3} :basin {} :domain {} :entries ({}))",
                crate::graph_sexp_escape(&act.concept),
                act.score,
                act.basin.to_sexp(),
                domain_to_sexp(act.domain),
                entries_sexp.join(" "),
            )
        }).collect();
        format!("(:ok :activations ({}))", items.join(" "))
    }

    /// Structural-only serialization: concept names + scores, no entry content.
    /// For progressive context injection round 1 — minimal tokens.
    pub fn to_sexp_structural(&self) -> String {
        let items: Vec<String> = self.activations.iter().map(|act| {
            format!(
                "(:concept \"{}\" :score {:.3} :basin {} :domain {})",
                crate::graph_sexp_escape(&act.concept),
                act.score,
                act.basin.to_sexp(),
                domain_to_sexp(act.domain),
            )
        }).collect();
        format!("(:ok :activations ({}))", items.join(" "))
    }
}

fn domain_to_sexp(d: Domain) -> &'static str {
    match d {
        Domain::Music => ":music",
        Domain::Math => ":math",
        Domain::Engineering => ":engineering",
        Domain::Cognitive => ":cognitive",
        Domain::Life => ":life",
        Domain::Generic => ":generic",
    }
}

// ── Core recall computation ──

/// Compute field recall — returns structured result.
/// The caller chooses how to serialize (full sexp, structural-only, etc.).
fn compute_recall(
    s: &mut FieldState,
    query_concepts: &[String],
    access_counts: &[(String, f64, f64)],
    limit: usize,
) -> RecallResult {
    let n = s.graph.n;
    if n == 0 {
        return RecallResult { activations: Vec::new() };
    }

    // Build source potential vector from query concepts.
    let sources = build_source_vector(&s.graph, query_concepts);

    // Solve the field: (L + εI)·φ = b.
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

    // Build access count vector with depth-aware temporal decay.
    let access_vec = build_access_vector(n, access_counts, &s.graph);

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

    // Update cycle.
    s.cycle += 1;

    // Map internal activations to structured results.
    let concept_activations: Vec<ConceptActivation> = activations.iter()
        .take(limit)
        .map(|act| {
            let node = &s.graph.nodes[act.node_index];
            let basin = if act.node_index < s.node_basins.len() {
                s.node_basins[act.node_index]
            } else {
                Basin::ThomasLobe(5) // fallback: generic domain
            };
            ConceptActivation {
                concept: node.concept.clone(),
                score: act.score,
                basin,
                domain: node.domain,
                entry_ids: node.entry_ids.clone(),
            }
        })
        .collect();

    RecallResult { activations: concept_activations }
}

/// Build per-node access count vector with temporal decay.
fn build_access_vector(
    n: usize,
    access_counts: &[(String, f64, f64)],
    graph: &crate::graph::SparseGraph,
) -> Vec<f64> {
    let decay_lambda = cfg_f64("decay-lambda", 0.01);
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let mut access_vec = vec![0.0; n];
    for (concept, count, last_access) in access_counts {
        if let Some(idx) = crate::graph::concept_index(graph, concept) {
            let age_hours = if *last_access > 0.0 {
                ((now_unix - last_access) / 3600.0).max(0.0)
            } else {
                0.0
            };
            let node_count = graph.nodes[idx].count as f64;
            let protection = 1.0 + node_count / 10.0;
            let decayed = (*count as f64).min(1.0) * (-decay_lambda * age_hours / protection).exp();
            access_vec[idx] = decayed;
        }
    }
    access_vec
}

// ── Public API ──

/// Full field recall — returns sexp string (backward compatible).
pub fn field_recall(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    access_counts: Vec<(String, f64, f64)>,
    limit: usize,
) -> Result<String, MemoryError> {
    let result = compute_recall(s, &query_concepts, &access_counts, limit);
    Ok(result.to_sexp())
}

/// Structural-only recall — concept names + scores + basins, no entry content.
/// For progressive context injection: ~10 tokens per concept vs ~50+ for full.
pub fn field_recall_structural(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    limit: usize,
) -> Result<String, MemoryError> {
    let result = compute_recall(s, &query_concepts, &[], limit);
    Ok(result.to_sexp_structural())
}

/// Current basin status — lightweight, no field solve.
pub fn current_basin(s: &FieldState) -> Result<String, MemoryError> {
    Ok(format!(
        "(:ok :basin {} :cycle {})",
        s.hysteresis.current_basin.to_sexp(),
        s.cycle,
    ))
}
