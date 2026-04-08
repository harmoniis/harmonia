/// Attractor stepping and basin management API.

use crate::attractor::{update_aizawa, update_halvorsen, update_thomas};
use crate::basin::{assign_node_basins, classify_primary_basin, update_hysteresis};
use crate::config::cfg_f64;
use crate::error::clamp;
use crate::graph::Domain;
use crate::FieldState;

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
