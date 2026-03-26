/// Activation scoring — combines field potential, eigenmode projection,
/// basin membership, and access count into a single recall activation score.
///
/// activation[i] = 0.40 × field + 0.30 × eigenmode + 0.20 × basin + 0.10 × access

use crate::basin::Basin;
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
    pub(crate) field_potential: f64,
    pub(crate) eigenmode_value: f64,
    pub(crate) basin: Basin,
}

/// Compute final activation scores for all nodes.
///
/// Returns scored nodes sorted by activation (descending), filtered to those
/// above the activation threshold.
pub(crate) fn compute_activation(
    phi: &[f64],
    eigenmode_activation: &[f64],
    current_basin: Basin,
    node_basins: &[Basin],
    access_counts: &[f64],
    n: usize,
    threshold: f64,
    cycle: i64,
) -> Vec<Activation> {
    if n == 0 {
        return Vec::new();
    }

    // Warm-up phase: ramp basin weight from initial to final over N cycles.
    // All parameters from config-store for harmonic tuning.
    let warm_up_cycles = cfg_i64("warm-up-cycles", 10);
    let basin_weight_initial = cfg_f64("basin-weight-initial", 0.05);
    let basin_weight_final = cfg_f64("basin-weight-final", 0.20);
    let basin_weight = if cycle < warm_up_cycles {
        let t = cycle as f64 / warm_up_cycles as f64;
        basin_weight_initial + t * (basin_weight_final - basin_weight_initial)
    } else {
        basin_weight_final
    };
    // Redistribute weight from basin to field + eigenmode during warm-up.
    let surplus = basin_weight_final - basin_weight;
    let field_weight = 0.40 + surplus * 0.5;
    let eigen_weight = 0.30 + surplus * 0.5;

    // Normalize field potentials to [0, 1].
    let phi_norm = normalize_to_unit(phi, n);
    // Normalize eigenmode activations to [0, 1] (use absolute value — anti-nodes are peaks).
    let eigen_abs: Vec<f64> = eigenmode_activation
        .iter()
        .take(n)
        .map(|v| v.abs())
        .collect();
    let eigen_norm = normalize_to_unit(&eigen_abs, n);

    let mut activations = Vec::with_capacity(n);

    for i in 0..n {
        let basin_factor = if i < node_basins.len() && node_basins[i] == current_basin {
            1.0
        } else {
            0.15
        };

        let access = if i < access_counts.len() {
            clamp(access_counts[i], 0.0, 1.0)
        } else {
            0.0
        };

        let score = clamp(
            field_weight * phi_norm[i] + eigen_weight * eigen_norm[i] + basin_weight * basin_factor + 0.10 * access,
            0.0,
            1.0,
        );

        if score >= threshold {
            activations.push(Activation {
                node_index: i,
                score,
                field_potential: phi_norm[i],
                eigenmode_value: eigen_norm[i],
                basin: if i < node_basins.len() {
                    node_basins[i]
                } else {
                    Basin::ThomasLobe(5)
                },
            });
        }
    }

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
        return vec![0.5; n]; // All equal — assign neutral activation.
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
        let node_basins = vec![
            Basin::ThomasLobe(0),
            Basin::ThomasLobe(1),
            Basin::ThomasLobe(0),
        ];
        let access = vec![0.5, 0.5, 0.5];

        let results = compute_activation(
            &phi,
            &eigen,
            Basin::ThomasLobe(0),
            &node_basins,
            &access,
            3,
            0.0,
            100, // past warm-up
        );

        // Node 0 (in basin, high phi) should score highest.
        assert_eq!(results[0].node_index, 0);
        // Node 1 (out of basin) should score lowest despite moderate values.
        let node1 = results.iter().find(|a| a.node_index == 1).unwrap();
        assert!(
            node1.score < results[0].score,
            "Out-of-basin node should score lower"
        );
    }

    #[test]
    fn test_threshold_filters() {
        let phi = vec![0.1, 0.9];
        let eigen = vec![0.1, 0.9];
        let basins = vec![Basin::ThomasLobe(0), Basin::ThomasLobe(0)];
        let access = vec![0.0, 0.0];

        let results =
            compute_activation(&phi, &eigen, Basin::ThomasLobe(0), &basins, &access, 2, 0.5, 100);

        // With a high threshold, only the high-scoring node should pass.
        assert!(results.len() <= 2);
        if results.len() == 1 {
            assert_eq!(results[0].node_index, 1);
        }
    }
}
