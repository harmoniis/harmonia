/// Dreaming -- field self-maintenance during idle.
///
/// Landauer's principle: erasing information has entropy cost k_B T ln(2) per bit.
/// Deletion is NOT free. The dreaming algorithm therefore:
///   1. PREFERS merging (compress) over pruning (delete)
///   2. Only prunes when K(m_i | graph \ m_i) ~= 0 (betweenness ~= 0, fully redundant)
///   3. Crystallizes structural nodes (promote depth, resist future decay)
///   4. Tracks entropy delta: dS = landauer_cost(pruned) - compression_gain(merged)

use crate::config::cfg_f64;
use crate::graph::betweenness_centrality;
use crate::spectral::{eigenmode_activate, eigenmode_project};
use crate::FieldState;

/// Returns DreamReport as sexp:
///   (:ok :pruned (...) :merged (...) :crystallized (...) :stats (:nodes N :entropy-delta F ...))
pub fn field_dream(s: &mut FieldState) -> Result<String, String> {
    let n = s.graph.n;
    if n == 0 {
        return Ok("(:ok :pruned () :merged () :crystallized () :stats (:nodes 0 :pruned 0 :merged 0 :crystallized 0 :entropy-delta 0.0))".into());
    }

    let prune_threshold = cfg_f64("dream-prune-threshold", 0.02);      // Very low -- only truly redundant
    let merge_threshold = cfg_f64("dream-merge-threshold", 0.15);       // Below this: merge, not delete
    let crystallize_threshold = cfg_f64("dream-crystallize-threshold", 0.80);

    // 1. Betweenness centrality -- structural importance (Kolmogorov proxy).
    let bc = betweenness_centrality(&s.graph);

    // 2. Quiescent eigenmode projection -- find the field's natural skeleton.
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
    //    dream_score = 0.5 * centrality + 0.5 * eigenmode_structural
    //
    //    score < prune_threshold  -> K(m|graph) ~= 0, safe to delete (Landauer cost minimal)
    //    score < merge_threshold  -> compress, don't delete (Landauer cost > 0)
    //    score > crystallize_threshold -> structural skeleton, promote depth
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
            // K(m|graph) ~= 0: node is on no shortest paths. Deletion cost is minimal.
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
            // Landauer says: merging preserves information, so entropy cost ~= 0.
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
            // Structural skeleton -- crystallize (promote depth).
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
