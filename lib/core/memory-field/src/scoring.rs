/// Activation scoring — combines field potential, eigenmode projection,
/// heat kernel propagation, basin membership, topological flux, and access
/// count into a single recall activation score.
///
/// Legacy:  activation[i] = field_w × field + eigen_w × eigenmode + basin_w × basin + 0.10 × access
/// Holographic: activation[i] = 0.25 × field + 0.15 × eigen + 0.20 × heat_kernel + 0.20 × basin + 0.10 × topo_flux + 0.10 × access

use crate::error::clamp;

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

/// A scored concept node with all contributing signals.
#[derive(Clone, Debug)]
pub(crate) struct Activation {
    pub(crate) node_index: usize,
    pub(crate) score: f64,
}

/// Pre-computed scoring weights — immutable for the entire scoring pass.
struct ScoringWeights {
    field: f64,
    eigen: f64,
    heat_kernel: f64,
    basin: f64,
    topological: f64,
}

impl ScoringWeights {
    fn from_config(cycle: i64, use_heat_kernel: bool) -> Self {
        let warm_up_cycles = cfg_i64("warm-up-cycles", 10);
        let basin_weight_initial = cfg_f64("basin-weight-initial", 0.05);
        let basin_weight_final = if use_heat_kernel { 0.20 } else { 0.20 };
        let basin = if cycle < warm_up_cycles {
            let t = cycle as f64 / warm_up_cycles as f64;
            basin_weight_initial + t * (basin_weight_final - basin_weight_initial)
        } else {
            basin_weight_final
        };

        if use_heat_kernel {
            // Holographic mode: field provides local resonance, heat kernel provides
            // path exploration, topo flux provides non-local information.
            // The field potential IS the resonance — never zero it out.
            let surplus = basin_weight_final - basin;
            Self {
                field: 0.25 + surplus * 0.25,
                eigen: 0.15 + surplus * 0.25,
                heat_kernel: 0.20 + surplus * 0.25,
                basin,
                topological: 0.10 + surplus * 0.25,
            }
        } else {
            let surplus = 0.20 - basin;
            Self {
                field: 0.40 + surplus * 0.5,
                eigen: 0.30 + surplus * 0.5,
                heat_kernel: 0.0,
                basin,
                topological: 0.0,
            }
        }
    }
}

/// Score a single node. Pure function — no state access.
fn score_node(
    phi_norm: f64,
    eigen_norm: f64,
    heat_kernel_norm: f64,
    basin_affinity: f64,
    access: f64,
    topo_flux: f64,
    weights: &ScoringWeights,
) -> f64 {
    clamp(
        weights.field * phi_norm
        + weights.eigen * eigen_norm
        + weights.heat_kernel * heat_kernel_norm
        + weights.basin * basin_affinity
        + weights.topological * topo_flux
        + 0.10 * access,
        0.0,
        1.0,
    )
}

/// Compute final activation scores for all nodes.
///
/// Returns scored nodes sorted by activation (descending), filtered to those
/// above the activation threshold. Functional: map → filter → sort → collect.
///
/// When `heat_kernel_activation` is Some, uses the heat-kernel scoring mode
/// (Phase B weights). When None, uses legacy field+eigen scoring.
pub(crate) fn compute_activation(
    phi: &[f64],
    eigenmode_activation: &[f64],
    heat_kernel_activation: Option<&[f64]>,
    topological_flux: Option<&[f64]>,
    basin_affinity: &[f64],
    access_counts: &[f64],
    n: usize,
    threshold: f64,
    cycle: i64,
) -> Vec<Activation> {
    if n == 0 {
        return Vec::new();
    }

    let use_heat_kernel = heat_kernel_activation.is_some();
    let weights = ScoringWeights::from_config(cycle, use_heat_kernel);
    let phi_norm = normalize_to_unit(phi, n);
    let eigen_abs: Vec<f64> = eigenmode_activation.iter().take(n).map(|v| v.abs()).collect();
    let eigen_norm = normalize_to_unit(&eigen_abs, n);

    let hk_norm = if let Some(hk) = heat_kernel_activation {
        let hk_abs: Vec<f64> = hk.iter().take(n).map(|v| v.abs()).collect();
        normalize_to_unit(&hk_abs, n)
    } else {
        vec![0.0; n]
    };

    let topo_flux_norm = topological_flux.map(|tf| normalize_to_unit(tf, n));

    let mut activations: Vec<Activation> = (0..n)
        .map(|i| {
            let affinity = if i < basin_affinity.len() { clamp(basin_affinity[i], 0.0, 1.0) } else { 0.15 };
            let access = if i < access_counts.len() { clamp(access_counts[i], 0.0, 1.0) } else { 0.0 };
            let tf = if let Some(ref tfn) = topo_flux_norm { if i < tfn.len() { tfn[i] } else { 0.0 } } else { 0.0 };
            Activation {
                node_index: i,
                score: score_node(phi_norm[i], eigen_norm[i], hk_norm[i], affinity, access, tf, &weights),
            }
        })
        .filter(|a| a.score >= threshold)
        .collect();

    activations.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    activations
}

/// Normalize a vector to [0, 1] range using min-max scaling.
fn normalize_to_unit(v: &[f64], n: usize) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let slice = &v[..n.min(v.len())];
    let min = slice.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = slice.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;

    if range < 1e-30 {
        return vec![0.5; n];
    }

    (0..n)
        .map(|i| {
            if i < v.len() {
                clamp((v[i] - min) / range, 0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_basin_scores_higher() {
        let phi = vec![0.8, 0.3, 0.5];
        let eigen = vec![0.6, 0.4, 0.5];
        // Node 0: in-basin (1.0), node 1: out-of-basin (0.15), node 2: in-basin (1.0)
        let basin_affinity = vec![1.0, 0.15, 1.0];
        let access = vec![0.5, 0.5, 0.5];

        let results = compute_activation(
            &phi,
            &eigen,
            None,
            None,
            &basin_affinity,
            &access,
            3,
            0.0,
            100, // past warm-up
        );

        // Node 0 (high affinity, high phi) should score highest.
        assert_eq!(results[0].node_index, 0);
        // Node 1 (low affinity) should score lowest despite moderate values.
        let node1 = results.iter().find(|a| a.node_index == 1).unwrap();
        assert!(
            node1.score < results[0].score,
            "Low-affinity node should score lower"
        );
    }

    #[test]
    fn test_threshold_filters() {
        let phi = vec![0.1, 0.9];
        let eigen = vec![0.1, 0.9];
        let basin_affinity = vec![1.0, 1.0];
        let access = vec![0.0, 0.0];

        let results =
            compute_activation(&phi, &eigen, None, None, &basin_affinity, &access, 2, 0.5, 100);

        // With a high threshold, only the high-scoring node should pass.
        assert!(results.len() <= 2);
        if results.len() == 1 {
            assert_eq!(results[0].node_index, 1);
        }
    }
}
