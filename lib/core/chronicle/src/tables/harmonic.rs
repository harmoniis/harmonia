use rusqlite::params;

use crate::db;

pub struct HarmonicSnapshot {
    pub cycle: i64,
    pub phase: String,
    pub strength: f64,
    pub utility: f64,
    pub beauty: f64,
    pub signal: f64,
    pub noise: f64,
    pub logistic_x: f64,
    pub logistic_r: f64,
    pub chaos_risk: f64,
    pub rewrite_aggression: f64,
    pub lorenz_x: f64,
    pub lorenz_y: f64,
    pub lorenz_z: f64,
    pub lorenz_radius: f64,
    pub lorenz_bounded: f64,
    pub lambdoma_global: f64,
    pub lambdoma_local: f64,
    pub lambdoma_ratio: f64,
    pub lambdoma_convergent: bool,
    pub rewrite_ready: bool,
    pub rewrite_count: i32,
    pub security_posture: String,
    pub security_events: i32,
    // Memory-field basin state for warm-start persistence.
    pub field_basin: String,
    pub field_coercive_energy: f64,
    pub field_dwell_ticks: i64,
    pub field_threshold: f64,
}

pub fn record(snap: &HarmonicSnapshot) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "INSERT INTO harmonic_snapshots
            (cycle, phase, strength, utility, beauty, signal, noise,
             logistic_x, logistic_r, chaos_risk, rewrite_aggression,
             lorenz_x, lorenz_y, lorenz_z, lorenz_radius, lorenz_bounded,
             lambdoma_global, lambdoma_local, lambdoma_ratio, lambdoma_convergent,
             rewrite_ready, rewrite_count, security_posture, security_events,
             field_basin, field_coercive_energy, field_dwell_ticks, field_threshold)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28)",
        params![
            snap.cycle, snap.phase, snap.strength, snap.utility, snap.beauty,
            snap.signal, snap.noise, snap.logistic_x, snap.logistic_r,
            snap.chaos_risk, snap.rewrite_aggression,
            snap.lorenz_x, snap.lorenz_y, snap.lorenz_z,
            snap.lorenz_radius, snap.lorenz_bounded,
            snap.lambdoma_global, snap.lambdoma_local, snap.lambdoma_ratio,
            snap.lambdoma_convergent as i32,
            snap.rewrite_ready as i32, snap.rewrite_count,
            snap.security_posture, snap.security_events,
            snap.field_basin, snap.field_coercive_energy,
            snap.field_dwell_ticks, snap.field_threshold,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Update the field_checkpoint column of the most recent harmonic snapshot.
/// Called from :stabilize after memory-field IPC checkpoint returns the full sexp.
pub fn update_field_checkpoint(checkpoint: &str) -> Result<(), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.execute(
        "UPDATE harmonic_snapshots SET field_checkpoint = ?1
         WHERE id = (SELECT MAX(id) FROM harmonic_snapshots)",
        params![checkpoint],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Query the last recorded field basin state for warm-start.
pub fn last_field_basin() -> Result<(String, f64, i64, f64), String> {
    let db = db::conn()?;
    let lock = db.lock().map_err(|e| e.to_string())?;
    lock.query_row(
        "SELECT field_basin, field_coercive_energy, field_dwell_ticks, field_threshold
         FROM harmonic_snapshots ORDER BY ts DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0).unwrap_or_else(|_| "thomas-0".into()),
                row.get::<_, f64>(1).unwrap_or(0.0),
                row.get::<_, i64>(2).unwrap_or(0),
                row.get::<_, f64>(3).unwrap_or(0.35),
            ))
        },
    )
    .map_err(|e| e.to_string())
}
