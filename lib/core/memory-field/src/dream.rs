/// Dreaming -- field self-maintenance during idle.
///
/// Landauer's principle: erasing information has entropy cost k_B T ln(2) per bit.
/// Deletion is NOT free. The dreaming algorithm therefore:
///   1. PREFERS merging (compress) over pruning (delete)
///   2. Only prunes when K(m_i | graph \ m_i) ~= 0 (betweenness ~= 0, fully redundant)
///   3. Crystallizes structural nodes (promote depth, resist future decay)
///   4. Tracks entropy delta: dS = landauer_cost(pruned) - compression_gain(merged)

use harmonia_actor_protocol::MemoryError;

use crate::config::cfg_f64;
use crate::graph::betweenness_centrality;
use crate::spectral::{eigenmode_activate, eigenmode_project};
use crate::FieldState;

// ── Configuration ──

struct DreamThresholds {
    prune: f64,
    merge: f64,
    crystallize: f64,
}

impl DreamThresholds {
    fn from_config() -> Self {
        Self {
            prune: cfg_f64("dream-prune-threshold", 0.02),
            merge: cfg_f64("dream-merge-threshold", 0.15),
            crystallize: cfg_f64("dream-crystallize-threshold", 0.80),
        }
    }
}

// ── Pure classification ──

/// Classification of a single node by the dream algorithm.
/// Pure function — no mutation, no side effects.
enum NodeClassification {
    /// K(m|graph) ~= 0: fully redundant, safe to delete (Landauer cost minimal).
    Prune {
        entry_ids: Vec<String>,
        entropy_cost: f64,
    },
    /// Some structural value but multiple entries — compress, don't delete.
    MergeCandidate {
        basin_key: String,
        node_idx: usize,
        entry_ids: Vec<String>,
    },
    /// Structural skeleton — promote depth, resist future decay.
    Crystallize {
        entry_ids: Vec<String>,
    },
    /// Not actionable — leave as is.
    Retain,
}

/// Classify a single node. Pure function: inputs → classification, no mutation.
fn classify_node(
    centrality: f64,
    eigen_norm: f64,
    entry_ids: &[String],
    basin_key: &str,
    node_idx: usize,
    thresholds: &DreamThresholds,
) -> NodeClassification {
    let dream_score = 0.5 * centrality + 0.5 * eigen_norm;
    let entry_count = entry_ids.len();

    if dream_score < thresholds.prune && centrality < 1e-10 && entry_count >= 2 {
        // Keep one entry (the last), prune the rest.
        let prune_count = entry_count - 1;
        let pruned: Vec<String> = entry_ids.iter().take(prune_count).cloned().collect();
        let entropy_cost = pruned.len() as f64 * (entry_count as f64).log2().max(1.0);
        NodeClassification::Prune { entry_ids: pruned, entropy_cost }
    } else if dream_score < thresholds.merge && entry_count >= 2 {
        NodeClassification::MergeCandidate {
            basin_key: basin_key.to_string(),
            node_idx,
            entry_ids: entry_ids.to_vec(),
        }
    } else if dream_score > thresholds.crystallize && !entry_ids.is_empty() {
        NodeClassification::Crystallize { entry_ids: entry_ids.to_vec() }
    } else {
        NodeClassification::Retain
    }
}

// ── Accumulation ──

/// Accumulated dream report from classification fold.
struct DreamReport {
    pruned_entries: Vec<String>,
    merge_groups: Vec<Vec<String>>,
    crystallized_entries: Vec<String>,
    entropy_delta: f64,
    node_count: usize,
}

/// Collect classifications into merge groups by basin, then build the report.
fn collect_dream_report(
    classifications: Vec<NodeClassification>,
    graph: &crate::graph::SparseGraph,
    n: usize,
) -> DreamReport {
    let mut pruned_entries = Vec::new();
    let mut crystallized_entries = Vec::new();
    let mut entropy_delta = 0.0;
    let mut basin_candidates: std::collections::HashMap<String, Vec<(usize, Vec<String>)>> =
        std::collections::HashMap::new();

    for classification in classifications {
        match classification {
            NodeClassification::Prune { entry_ids, entropy_cost } => {
                pruned_entries.extend(entry_ids);
                entropy_delta += entropy_cost;
            }
            NodeClassification::MergeCandidate { basin_key, node_idx, entry_ids } => {
                basin_candidates.entry(basin_key).or_default().push((node_idx, entry_ids));
            }
            NodeClassification::Crystallize { entry_ids } => {
                crystallized_entries.extend(entry_ids);
                entropy_delta -= 1.0; // Crystallization reduces field entropy.
            }
            NodeClassification::Retain => {}
        }
    }

    // Build merge groups: nodes in same basin merge their entries.
    let merge_groups: Vec<Vec<String>> = basin_candidates
        .into_values()
        .filter_map(|candidates| {
            let group: Vec<String> = candidates
                .into_iter()
                .flat_map(|(node_idx, eids)| {
                    // Use graph node entry_ids for canonical ordering.
                    if node_idx < graph.nodes.len() {
                        graph.nodes[node_idx].entry_ids.clone()
                    } else {
                        eids
                    }
                })
                .collect();
            (group.len() >= 2).then_some(group)
        })
        .collect();

    DreamReport { pruned_entries, merge_groups, crystallized_entries, entropy_delta, node_count: n }
}

// ── Sexp serialization (separated from logic) ──

fn dream_report_to_sexp(report: &DreamReport) -> String {
    let escape = crate::graph_sexp_escape;

    let pruned_sexp: Vec<String> = report.pruned_entries
        .iter()
        .map(|e| format!("\"{}\"", escape(e)))
        .collect();

    let cryst_sexp: Vec<String> = report.crystallized_entries
        .iter()
        .map(|e| format!("\"{}\"", escape(e)))
        .collect();

    let merged_sexp: Vec<String> = report.merge_groups
        .iter()
        .map(|group| {
            let ids: Vec<String> = group.iter().map(|e| format!("\"{}\"", escape(e))).collect();
            format!("({})", ids.join(" "))
        })
        .collect();

    format!(
        "(:ok :pruned ({}) :merged ({}) :crystallized ({}) :stats (:nodes {} :pruned {} :merged {} :crystallized {} :entropy-delta {:.3}))",
        pruned_sexp.join(" "),
        merged_sexp.join(" "),
        cryst_sexp.join(" "),
        report.node_count,
        report.pruned_entries.len(),
        report.merge_groups.len(),
        report.crystallized_entries.len(),
        report.entropy_delta,
    )
}

// ── Public API ──

/// Field dreaming: classify → collect → serialize.
/// Returns DreamReport as sexp.
pub fn field_dream(s: &mut FieldState) -> Result<String, MemoryError> {
    let n = s.graph.n;
    if n == 0 {
        return Ok("(:ok :pruned () :merged () :crystallized () :stats (:nodes 0 :pruned 0 :merged 0 :crystallized 0 :entropy-delta 0.0))".into());
    }

    let thresholds = DreamThresholds::from_config();

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

    // 3. Classify each node — pure functional map.
    let classifications: Vec<NodeClassification> = (0..n)
        .map(|i| {
            let eigen_norm = if es_range > 1e-30 {
                (eigen_structural[i] - es_min) / es_range
            } else {
                0.5
            };
            let basin_key = if i < s.node_basins.len() {
                s.node_basins[i].to_sexp()
            } else {
                "unknown".to_string()
            };
            classify_node(bc[i], eigen_norm, &s.graph.nodes[i].entry_ids, &basin_key, i, &thresholds)
        })
        .collect();

    // 4. Collect classifications into dream report — functional fold.
    let report = collect_dream_report(classifications, &s.graph, n);

    // 5. Serialize — pure presentation, separated from logic.
    Ok(dream_report_to_sexp(&report))
}
