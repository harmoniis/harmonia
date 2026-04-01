/// Public API surface for the memory field engine.
///
/// All functions take &mut FieldState — the actor owns the state.
/// No globals, no singletons, no C FFI.

use crate::attractor::{update_aizawa, update_halvorsen, update_thomas};
use crate::basin::{assign_node_basins, classify_primary_basin, update_hysteresis};
use crate::error::clamp;
use crate::field::{build_source_vector, solve_field};
use crate::graph::{betweenness_centrality, build_graph, Domain};
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
    // access_decayed = count × exp(-λ × age_hours / protection)
    // protection = 1 + node.count/10 (more connections → more structural → slower decay)
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
                0.0 // No last-access info → no decay (treat as fresh)
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

/// Dreaming — field self-maintenance during idle.
///
/// Landauer's principle: erasing information has entropy cost k_B T ln(2) per bit.
/// Deletion is NOT free. The dreaming algorithm therefore:
///   1. PREFERS merging (compress) over pruning (delete)
///   2. Only prunes when K(mᵢ | graph \ mᵢ) ≈ 0 (betweenness ≈ 0, fully redundant)
///   3. Crystallizes structural nodes (promote depth, resist future decay)
///   4. Tracks entropy delta: ΔS = landauer_cost(pruned) - compression_gain(merged)
///
/// Returns DreamReport as sexp:
///   (:ok :pruned (...) :merged (...) :crystallized (...) :stats (:nodes N :entropy-delta F ...))
pub fn field_dream(s: &mut FieldState) -> Result<String, String> {
    let n = s.graph.n;
    if n == 0 {
        return Ok("(:ok :pruned () :merged () :crystallized () :stats (:nodes 0 :pruned 0 :merged 0 :crystallized 0 :entropy-delta 0.0))".into());
    }

    let prune_threshold = cfg_f64("dream-prune-threshold", 0.02);      // Very low — only truly redundant
    let merge_threshold = cfg_f64("dream-merge-threshold", 0.15);       // Below this: merge, not delete
    let crystallize_threshold = cfg_f64("dream-crystallize-threshold", 0.80);

    // 1. Betweenness centrality — structural importance (Kolmogorov proxy).
    let bc = betweenness_centrality(&s.graph);

    // 2. Quiescent eigenmode projection — find the field's natural skeleton.
    let eigen_structural: Vec<f64> = if !s.eigenvectors.is_empty() {
        let uniform: Vec<f64> = vec![1.0 / n as f64; n];
        let projections = eigenmode_project(&uniform, &s.eigenvectors);
        eigenmode_activate(&projections, &s.eigenvectors, n)
            .into_iter()
            .map(|v| v.abs())
            .collect()
    } else {
        vec![0.5; n]
    };

    // Normalize eigen_structural to [0, 1].
    let es_max = eigen_structural.iter().cloned().fold(0.0_f64, f64::max);
    let es_min = eigen_structural.iter().cloned().fold(f64::INFINITY, f64::min);
    let es_range = es_max - es_min;

    // 3. Classify each node by dream_score.
    //    dream_score = 0.5 × centrality + 0.5 × eigenmode_structural
    //
    //    score < prune_threshold  → K(m|graph) ≈ 0, safe to delete (Landauer cost minimal)
    //    score < merge_threshold  → compress, don't delete (Landauer cost > 0)
    //    score > crystallize_threshold → structural skeleton, promote depth
    let mut pruned_entries: Vec<String> = Vec::new();
    let mut merge_groups: Vec<Vec<String>> = Vec::new();
    let mut crystallized_entries: Vec<String> = Vec::new();
    let mut entropy_delta: f64 = 0.0;

    // Collect merge candidates by basin (same basin = semantically related).
    let mut basin_merge_candidates: std::collections::HashMap<String, Vec<(usize, f64)>> =
        std::collections::HashMap::new();

    for i in 0..n {
        let centrality = bc[i];
        let eigen_norm = if es_range > 1e-30 {
            (eigen_structural[i] - es_min) / es_range
        } else {
            0.5
        };
        let dream_score = 0.5 * centrality + 0.5 * eigen_norm;
        let node = &s.graph.nodes[i];

        if dream_score < prune_threshold && centrality < 1e-10 && node.entry_ids.len() >= 2 {
            // K(m|graph) ≈ 0: node is on no shortest paths. Deletion cost is minimal.
            // Keep one entry (the last), prune the rest.
            let prune_count = node.entry_ids.len() - 1;
            for eid in node.entry_ids.iter().take(prune_count) {
                pruned_entries.push(eid.clone());
                // Landauer cost: log2(entry_count) bits per pruned entry (approximate).
                entropy_delta += (node.entry_ids.len() as f64).log2().max(1.0);
            }
        } else if dream_score < merge_threshold && node.entry_ids.len() >= 2 {
            // Node has some structural value but multiple entries.
            // Compress: merge entries into one (Lisp side creates combined entry).
            // Landauer says: merging preserves information, so entropy cost ≈ 0.
            let basin_key = if i < s.node_basins.len() {
                s.node_basins[i].to_sexp()
            } else {
                "unknown".to_string()
            };
            basin_merge_candidates
                .entry(basin_key)
                .or_default()
                .push((i, dream_score));
        } else if dream_score > crystallize_threshold && !node.entry_ids.is_empty() {
            // Structural skeleton — crystallize (promote depth).
            // This REDUCES future entropy by protecting against decay.
            for eid in &node.entry_ids {
                crystallized_entries.push(eid.clone());
            }
            entropy_delta -= 1.0; // Crystallization reduces field entropy.
        }
    }

    // Build merge groups: nodes in same basin with low scores merge their entries.
    for (_basin, candidates) in &basin_merge_candidates {
        let mut group_entries: Vec<String> = Vec::new();
        for &(node_idx, _score) in candidates {
            let node = &s.graph.nodes[node_idx];
            for eid in &node.entry_ids {
                group_entries.push(eid.clone());
            }
        }
        if group_entries.len() >= 2 {
            merge_groups.push(group_entries);
        }
    }

    // Format as sexp.
    let pruned_sexp: Vec<String> = pruned_entries
        .iter()
        .map(|e| format!("\"{}\"", crate::graph_sexp_escape(e)))
        .collect();
    let cryst_sexp: Vec<String> = crystallized_entries
        .iter()
        .map(|e| format!("\"{}\"", crate::graph_sexp_escape(e)))
        .collect();
    let merged_sexp: Vec<String> = merge_groups
        .iter()
        .map(|group| {
            let ids: Vec<String> = group
                .iter()
                .map(|e| format!("\"{}\"", crate::graph_sexp_escape(e)))
                .collect();
            format!("({})", ids.join(" "))
        })
        .collect();

    Ok(format!(
        "(:ok :pruned ({}) :merged ({}) :crystallized ({}) :stats (:nodes {} :pruned {} :merged {} :crystallized {} :entropy-delta {:.3}))",
        pruned_sexp.join(" "),
        merged_sexp.join(" "),
        cryst_sexp.join(" "),
        n,
        pruned_entries.len(),
        merge_groups.len(),
        crystallized_entries.len(),
        entropy_delta,
    ))
}
