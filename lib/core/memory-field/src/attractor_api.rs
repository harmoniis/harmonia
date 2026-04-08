/// Attractor stepping and basin management API.

use harmonia_actor_protocol::MemoryError;

// Pure step functions used by compute_step_pure; update_* wrappers no longer needed here.
use crate::basin::{assign_node_basins, classify_primary_basin, update_hysteresis};
use crate::config::cfg_f64;
use crate::error::clamp;
use crate::graph::Domain;
use crate::FieldState;

/// Pure attractor step computation — takes &FieldState (immutable), returns all new values.
/// State mutation is handled by FieldDelta::AttractorStepped in the Service pattern.
pub(crate) fn compute_step_pure(
    s: &FieldState,
    signal: f64,
    noise: f64,
) -> (
    crate::attractor::ThomasState,
    f64,
    crate::attractor::AizawaState,
    crate::attractor::HalvorsenState,
    crate::basin::HysteresisTracker,
    Vec<crate::basin::Basin>,
) {
    use crate::attractor::{step_thomas, step_aizawa, step_halvorsen};

    // Thomas b parameter modulated by signal quality (all from config).
    let b_base = cfg_f64("thomas-b-base", 0.208);
    let b_scale = cfg_f64("thomas-b-modulation-scale", 0.02);
    let b_min = cfg_f64("thomas-b-min", 0.18);
    let b_max = cfg_f64("thomas-b-max", 0.24);
    let b_eff = clamp(b_base + b_scale * (signal - noise), b_min, b_max);

    let thomas_dt = cfg_f64("thomas-dt", 0.05);
    let aizawa_dt = cfg_f64("aizawa-dt", 0.01);
    let halvorsen_dt = cfg_f64("halvorsen-dt", 0.01);
    let new_thomas = step_thomas(&s.thomas, b_eff, thomas_dt);
    let new_aizawa = step_aizawa(&s.aizawa, aizawa_dt);
    let new_halvorsen = step_halvorsen(&s.halvorsen, halvorsen_dt);

    // Update basin assignment and hysteresis (on a clone).
    let proposed = classify_primary_basin(&new_thomas);
    let drive_energy = (signal - noise).abs() * 0.1;
    let mut new_hysteresis = s.hysteresis.clone();
    let _switched = update_hysteresis(&mut new_hysteresis, proposed, drive_energy);

    // Re-assign node basins if we have a graph.
    let new_node_basins = if s.graph.n > 0 {
        let domains: Vec<Domain> = s.graph.nodes.iter().map(|n| n.domain).collect();
        assign_node_basins(&domains, &new_thomas, &new_aizawa, &new_halvorsen)
    } else {
        s.node_basins.clone()
    };

    (new_thomas, b_eff, new_aizawa, new_halvorsen, new_hysteresis, new_node_basins)
}

/// Step all three attractors by one timestep and update hysteresis.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn step_attractors(s: &mut FieldState, signal: f64, noise: f64) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::{FieldCommand, FieldResult};
    let cmd = FieldCommand::StepAttractors { signal, noise };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    match result {
        FieldResult::Stepped(r) => Ok(r.to_sexp()),
        _ => unreachable!(),
    }
}

/// Return current basin status as sexp.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn basin_status(s: &FieldState) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::BasinStatus;
    let (_delta, result) = s.handle(cmd)?;
    Ok(result.to_sexp())
}

/// Restore basin state from Chronicle for warm-start.
/// Backward-compat wrapper: delegates through the Service pattern.
pub fn restore_basin(
    s: &mut FieldState,
    basin_str: &str,
    coercive_energy: f64,
    dwell_ticks: u64,
    threshold: f64,
) -> Result<String, MemoryError> {
    use harmonia_actor_protocol::Service;
    use crate::command::FieldCommand;
    let cmd = FieldCommand::RestoreBasin {
        basin_str: basin_str.to_string(),
        coercive_energy,
        dwell_ticks,
        threshold,
    };
    let (delta, result) = s.handle(cmd)?;
    s.apply(delta);
    Ok(result.to_sexp())
}
