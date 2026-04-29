/// Field-based recall — solve the Laplacian field for query concepts.

use harmonia_actor_protocol::MemoryError;

use crate::basin::Basin;
use crate::config::{cfg_f64, cfg_i64};
use crate::error::clamp;
use crate::field::{build_source_vector, solve_field};
use crate::graph::Domain;
use crate::scoring::compute_activation;
use crate::spectral::{eigenmode_activate, eigenmode_project};
use crate::FieldState;
use crate::basin::compute_basin_affinity;

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

/// Compute heat kernel diffusion time from signal-noise ratio.
///
/// High signal → small t → precise local recall
/// Low signal → large t → broad associative recall
///
/// This is the uncertainty principle analog: precision in concept space
/// trades off with breadth of association.
fn compute_diffusion_time(signal: f64, noise: f64) -> f64 {
    let t_min = cfg_f64("heat-kernel-t-min", 0.1);
    let t_max = cfg_f64("heat-kernel-t-max", 5.0);
    let snr = clamp(signal - noise, 0.0, 1.0);
    // High SNR → small t (local), low SNR → large t (global)
    t_max - snr * (t_max - t_min)
}

// ── Core recall computation ──

/// Pure recall computation — takes &FieldState (immutable), returns structured result
/// plus a scalar eigenmode coherence (energy fraction in dominant modes).
/// The cycle increment + coherence persistence are applied by the Service pattern.
pub(crate) fn compute_recall_pure(
    s: &FieldState,
    query_concepts: &[String],
    access_counts: &[(String, f64, f64)],
    limit: usize,
) -> (RecallResult, f64) {
    let n = s.graph.n;
    if n == 0 {
        return (RecallResult { activations: Vec::new() }, 0.0);
    }

    // Build source potential vector from query concepts.
    let sources = build_source_vector(&s.graph, query_concepts);

    // Solve the field: (L + εI)·φ = b.
    let max_iter = cfg_i64("solver-max-iter", 50) as usize;
    let tol = cfg_f64("solver-tol", 0.001);
    let epsilon = cfg_f64("solver-epsilon", 0.01);
    let phi = solve_field(&s.graph, &sources, max_iter, tol, epsilon);

    // Compute eigenmode activation (Chladni projection).
    // The projections vector ⟨b, v_k⟩ is the energy distribution across modes;
    // coherence is the fraction in the dominant (lowest-λ) two modes.
    let (eigenmode_activation, coherence) = if !s.eigenvectors.is_empty() {
        let projections = eigenmode_project(&sources, &s.eigenvectors);
        let coh = compute_eigenmode_coherence(&projections);
        let act = eigenmode_activate(&projections, &s.eigenvectors, n);
        (act, coh)
    } else {
        (vec![0.0; n], 0.0)
    };

    // Heat kernel: enabled by default — the holographic propagator.
    // Uses signal/noise from the most recent attractor step (signalograd coupling).
    let heat_kernel_act = if !s.eigenvectors.is_empty() && !s.eigenvalues.is_empty() {
        let t = compute_diffusion_time(s.last_signal, s.last_noise);
        Some(crate::spectral::heat_kernel_activate(
            &sources, &s.eigenvalues, &s.eigenvectors, t, n,
        ))
    } else {
        None
    };

    // Topological flux — the A-B invariant: non-local information from graph cycles.
    let topo_flux = if !s.topology.node_flux.is_empty() {
        Some(s.topology.node_flux.as_slice())
    } else {
        None
    };

    // Compute continuous basin affinity (holographic projection from soft classification).
    // Replaces the binary in-basin/out-of-basin gate with soft Boltzmann probability.
    let basin_affinity = compute_basin_affinity(
        &s.graph.nodes.iter().map(|n| n.domain).collect::<Vec<_>>(),
        &s.thomas_soft_basins,
        &s.aizawa,
        &s.halvorsen,
    );

    // Build access count vector with depth-aware temporal decay.
    let access_vec = build_access_vector(n, access_counts, &s.graph);

    // Score all nodes — holographic fusion of all boundary and bulk signals.
    let activations = compute_activation(
        &phi,
        &eigenmode_activation,
        heat_kernel_act.as_deref(),
        topo_flux,
        &basin_affinity,
        &access_vec,
        n,
        cfg_f64("activation-threshold", 0.1),
        s.cycle,
    );

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

    (RecallResult { activations: concept_activations }, coherence)
}

/// Eigenmode coherence: fraction of source energy concentrated in the dominant
/// (smallest-eigenvalue) two modes. 1.0 means recall is sharply tied to a single
/// global structure; 0.0 means energy is fully diffuse across all modes.
/// Returns 0.0 for empty / zero-norm source vectors.
fn compute_eigenmode_coherence(projections: &[f64]) -> f64 {
    let total: f64 = projections.iter().map(|p| p * p).sum();
    if total <= f64::EPSILON || projections.is_empty() {
        return 0.0;
    }
    let take = projections.len().min(2);
    let dominant: f64 = projections.iter().take(take).map(|p| p * p).sum();
    (dominant / total).clamp(0.0, 1.0)
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

/// Full field recall — returns structured RecallResult.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn field_recall(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    access_counts: Vec<(String, f64, f64)>,
    limit: usize,
) -> Result<RecallResult, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::{FieldCommand, FieldResult};
    let cmd = FieldCommand::Recall { query_concepts, access_counts, limit };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    match result {
        FieldResult::Recalled(r) => Ok(r),
        _ => unreachable!(),
    }
}

/// Structural-only recall — concept names + scores + basins, no entry content.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn field_recall_structural(
    s: &mut FieldState,
    query_concepts: Vec<String>,
    limit: usize,
) -> Result<RecallResult, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::{FieldCommand, FieldResult};
    let cmd = FieldCommand::RecallStructural { query_concepts, limit };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    match result {
        FieldResult::Recalled(r) => Ok(r),
        _ => unreachable!(),
    }
}

/// Current basin status — lightweight, no field solve.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn current_basin(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::CurrentBasin;
    let (_delta, result) = s.handle(cmd)?;
    Ok(result.to_sexp())
}
