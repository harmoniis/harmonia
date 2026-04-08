/// Basin assignment and hysteresis tracking.
///
/// Attractor basins partition the concept graph into dynamical regimes.
/// Hysteresis ensures that weak signals don't trigger basin switches —
/// only strong, sustained context drives the system across an energy barrier.

use crate::attractor::{
    classify_aizawa_depth, classify_halvorsen_lobe, BasinClassifier, AizawaState,
    HalvorsenState, ThomasState,
};
use crate::error::clamp;
use crate::graph::Domain;

/// Which attractor basin a concept node currently belongs to.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Basin {
    /// Thomas lobe 0-5, one per domain.
    ThomasLobe(u8),
    /// Aizawa surface — shallow memories.
    AizawaSurface,
    /// Aizawa tube — deep crystallized memories.
    AizawaTube,
    /// Halvorsen lobe 0-2, interdisciplinary bridge clusters.
    HalvorsenLobe(u8),
}

impl Basin {
    pub(crate) fn from_sexp(s: &str) -> Self {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix(":thomas-") {
            let n = rest.parse::<u8>().unwrap_or(0).min(5);
            Basin::ThomasLobe(n)
        } else if s == ":aizawa-surface" {
            Basin::AizawaSurface
        } else if s == ":aizawa-tube" {
            Basin::AizawaTube
        } else if let Some(rest) = s.strip_prefix(":halvorsen-") {
            let n = rest.parse::<u8>().unwrap_or(0).min(2);
            Basin::HalvorsenLobe(n)
        } else {
            Basin::ThomasLobe(0)
        }
    }

    pub(crate) fn to_sexp(&self) -> String {
        match self {
            Basin::ThomasLobe(n) => format!(":thomas-{n}"),
            Basin::AizawaSurface => ":aizawa-surface".into(),
            Basin::AizawaTube => ":aizawa-tube".into(),
            Basin::HalvorsenLobe(n) => format!(":halvorsen-{n}"),
        }
    }
}

/// Hysteresis state for basin switching.
#[derive(Clone, Debug)]
pub(crate) struct HysteresisTracker {
    pub(crate) current_basin: Basin,
    pub(crate) coercive_energy: f64,
    pub(crate) threshold: f64,
    pub(crate) dwell_ticks: u64,
}

impl Default for HysteresisTracker {
    fn default() -> Self {
        Self {
            current_basin: Basin::ThomasLobe(0),
            coercive_energy: 0.0,
            threshold: 0.35,
            dwell_ticks: 0,
        }
    }
}

impl HysteresisTracker {
    /// Restore basin state from persisted Chronicle values (warm-start).
    pub(crate) fn restored(basin: Basin, energy: f64, dwell: u64, threshold: f64) -> Self {
        Self {
            current_basin: basin,
            coercive_energy: energy,
            threshold,
            dwell_ticks: dwell,
        }
    }
}

/// Map a concept-graph Domain to the corresponding Thomas basin index.
pub(crate) fn domain_to_thomas_basin(domain: Domain) -> u8 {
    domain.index().min(5)
}

/// Determine the current primary basin from all three attractor states.
///
/// The Thomas attractor provides the primary basin (domain routing).
/// Aizawa provides depth overlay. Halvorsen provides bridge overlay.
/// The primary basin used for hysteresis is the Thomas basin.
///
/// Uses the `BasinClassifier` trait so any attractor type can be substituted.
pub(crate) fn classify_primary_basin(thomas: &ThomasState) -> Basin {
    Basin::ThomasLobe(thomas.classify_basin())
}

/// Assign each concept node a basin based on its domain and attractor state.
pub(crate) fn assign_node_basins(
    domains: &[Domain],
    _thomas: &ThomasState,
    aizawa: &AizawaState,
    halvorsen: &HalvorsenState,
) -> Vec<Basin> {
    let is_deep = classify_aizawa_depth(aizawa);
    let _h_lobe = classify_halvorsen_lobe(halvorsen);

    domains
        .iter()
        .map(|domain| {
            if is_deep {
                // When Aizawa is in the tube, crystallized memories are active.
                Basin::AizawaTube
            } else {
                // Normal mode: domain-based Thomas basin.
                Basin::ThomasLobe(domain_to_thomas_basin(*domain))
            }
        })
        .collect()
}

/// Hysteresis parameters — all from config-store with sensible defaults.
fn cfg_f64(key: &str, default: f64) -> f64 {
    harmonia_config_store::get_own("memory-field", key)
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(default)
}

/// Update hysteresis tracker given the current attractor states.
///
/// Returns true if a basin switch occurred.
pub(crate) fn update_hysteresis(
    tracker: &mut HysteresisTracker,
    proposed_basin: Basin,
    drive_energy: f64,
) -> bool {
    let threshold_base = cfg_f64("hysteresis-threshold-base", 0.35);
    let threshold_scale = cfg_f64("hysteresis-threshold-scale", 0.15);
    let dwell_timescale = cfg_f64("hysteresis-dwell-timescale", 20.0);
    let decay = cfg_f64("hysteresis-decay", 0.92);

    tracker.dwell_ticks += 1;

    // Dynamic threshold: longer dwell = harder to switch.
    tracker.threshold = threshold_base
        + threshold_scale * (tracker.dwell_ticks as f64 / (tracker.dwell_ticks as f64 + dwell_timescale));

    if proposed_basin != tracker.current_basin {
        tracker.coercive_energy += drive_energy;
    }

    // Decay coercive energy each tick (relaxation).
    tracker.coercive_energy = clamp(tracker.coercive_energy * decay, 0.0, 5.0);

    if tracker.coercive_energy > tracker.threshold {
        tracker.current_basin = proposed_basin;
        tracker.coercive_energy = 0.0;
        tracker.dwell_ticks = 0;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weak_signal_no_switch() {
        let mut tracker = HysteresisTracker::default();
        // Apply weak drive energy — should not switch.
        for _ in 0..5 {
            let switched = update_hysteresis(&mut tracker, Basin::ThomasLobe(3), 0.01);
            assert!(!switched, "Weak signal should not cause basin switch");
        }
        assert_eq!(tracker.current_basin, Basin::ThomasLobe(0));
    }

    #[test]
    fn test_strong_sustained_signal_switches() {
        let mut tracker = HysteresisTracker::default();
        let mut switched = false;
        // Apply strong drive energy repeatedly.
        for _ in 0..50 {
            if update_hysteresis(&mut tracker, Basin::ThomasLobe(3), 0.5) {
                switched = true;
                break;
            }
        }
        assert!(switched, "Strong sustained signal should cause basin switch");
        assert_eq!(tracker.current_basin, Basin::ThomasLobe(3));
    }

    #[test]
    fn test_same_basin_no_accumulation() {
        let mut tracker = HysteresisTracker::default();
        // Proposing the same basin should not accumulate energy.
        update_hysteresis(&mut tracker, Basin::ThomasLobe(0), 1.0);
        // Energy only added when proposed differs from current,
        // but decay still applies.
        assert!(tracker.coercive_energy < 1.0);
    }
}
