/// Attractor dynamics for memory basin assignment.
///
/// Three attractor families provide distinct basin geometries:
/// - Thomas: cyclically symmetric, up to 6 coexisting basins for multi-domain routing
/// - Aizawa: sphere + tube topology for depth recall (shallow vs crystal)
/// - Halvorsen: 3-lobed propeller for interdisciplinary bridging
///
/// These live in memory-field, separate from Signalograd's Lorenz attractor,
/// maintaining clean separation of concerns.

use crate::error::clamp;

// ─── Thomas Attractor ───────────────────────────────────────────────────────

/// Thomas attractor state (cyclically symmetric chaotic system).
///
/// dx/dt = sin(y) - b·x
/// dy/dt = sin(z) - b·y
/// dz/dt = sin(x) - b·z
///
/// At b ≈ 0.208 the system has maximum coexisting attractors (up to 6),
/// ideal for multi-domain memory routing. The cyclic symmetry models
/// biological feedback loops (A→B→C→A).
#[derive(Clone, Debug)]
pub(crate) struct ThomasState {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl Default for ThomasState {
    fn default() -> Self {
        Self {
            x: 0.1,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Step the Thomas attractor by dt.
pub(crate) fn update_thomas(thomas: &mut ThomasState, b: f64, dt: f64) {
    let dx = thomas.y.sin() - b * thomas.x;
    let dy = thomas.z.sin() - b * thomas.y;
    let dz = thomas.x.sin() - b * thomas.z;
    thomas.x = clamp(thomas.x + dt * dx, -3.0, 3.0);
    thomas.y = clamp(thomas.y + dt * dy, -3.0, 3.0);
    thomas.z = clamp(thomas.z + dt * dz, -3.0, 3.0);
}

/// Classify which of 6 Thomas basins the state is in.
/// Uses the sign pattern of (x, y, z) to partition into octant-pairs,
/// then maps to basin indices 0-5 (one per domain).
pub(crate) fn classify_thomas_basin(thomas: &ThomasState) -> u8 {
    let sx = if thomas.x >= 0.0 { 1u8 } else { 0 };
    let sy = if thomas.y >= 0.0 { 1u8 } else { 0 };
    let sz = if thomas.z >= 0.0 { 1u8 } else { 0 };
    // Map 8 octants to 6 basins (cyclic symmetry collapses antipodal pairs).
    match (sx, sy, sz) {
        (1, 1, 1) => 0,
        (0, 0, 0) => 0, // antipodal
        (1, 1, 0) => 1,
        (0, 0, 1) => 1,
        (1, 0, 1) => 2,
        (0, 1, 0) => 2,
        (1, 0, 0) => 3,
        (0, 1, 1) => 3,
        _ => 5, // unreachable, but safe fallback
    }
}

// ─── Aizawa Attractor ───────────────────────────────────────────────────────

/// Aizawa attractor state (Langford system).
///
/// dx/dt = (z - b)·x - d·y
/// dy/dt = d·x + (z - b)·y
/// dz/dt = c + a·z - z³/3 - (x² + y²)·(1 + e·z) + f·z·x³
///
/// Sphere + tube topology: shallow memories orbit the surface,
/// deep crystals inhabit the tube channel.
#[derive(Clone, Debug)]
pub(crate) struct AizawaState {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl Default for AizawaState {
    fn default() -> Self {
        Self {
            x: 0.1,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Aizawa parameters (standard values from Langford 1984).
const AIZAWA_A: f64 = 0.95;
const AIZAWA_B: f64 = 0.7;
const AIZAWA_C: f64 = 0.6;
const AIZAWA_D: f64 = 3.5;
const AIZAWA_E: f64 = 0.25;
const AIZAWA_F: f64 = 0.1;

/// Step the Aizawa attractor by dt.
pub(crate) fn update_aizawa(aizawa: &mut AizawaState, dt: f64) {
    let x = aizawa.x;
    let y = aizawa.y;
    let z = aizawa.z;
    let r2 = x * x + y * y;

    let dx = (z - AIZAWA_B) * x - AIZAWA_D * y;
    let dy = AIZAWA_D * x + (z - AIZAWA_B) * y;
    let dz = AIZAWA_C + AIZAWA_A * z - z * z * z / 3.0 - r2 * (1.0 + AIZAWA_E * z)
        + AIZAWA_F * z * x * x * x;

    aizawa.x = clamp(x + dt * dx, -3.0, 3.0);
    aizawa.y = clamp(y + dt * dy, -3.0, 3.0);
    aizawa.z = clamp(z + dt * dz, -3.0, 3.0);
}

/// Classify Aizawa depth: tube (|z| > threshold) vs surface.
pub(crate) fn classify_aizawa_depth(aizawa: &AizawaState) -> bool {
    // true = tube (deep/crystal), false = surface (shallow)
    aizawa.z.abs() > 1.5
}

// ─── Halvorsen Attractor ────────────────────────────────────────────────────

/// Halvorsen attractor state (cyclically symmetric, 3-lobed propeller).
///
/// dx/dt = -a·x - 4·y - 4·z - y²
/// dy/dt = -a·y - 4·z - 4·x - z²
/// dz/dt = -a·z - 4·x - 4·y - x²
///
/// Three interconnected lobes model interdisciplinary bridging.
#[derive(Clone, Debug)]
pub(crate) struct HalvorsenState {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

impl Default for HalvorsenState {
    fn default() -> Self {
        Self {
            x: 0.1,
            y: 0.0,
            z: 0.0,
        }
    }
}

const HALVORSEN_A: f64 = 1.89;

/// Step the Halvorsen attractor by dt.
pub(crate) fn update_halvorsen(halvorsen: &mut HalvorsenState, dt: f64) {
    let x = halvorsen.x;
    let y = halvorsen.y;
    let z = halvorsen.z;

    let dx = -HALVORSEN_A * x - 4.0 * y - 4.0 * z - y * y;
    let dy = -HALVORSEN_A * y - 4.0 * z - 4.0 * x - z * z;
    let dz = -HALVORSEN_A * z - 4.0 * x - 4.0 * y - x * x;

    halvorsen.x = clamp(x + dt * dx, -15.0, 15.0);
    halvorsen.y = clamp(y + dt * dy, -15.0, 15.0);
    halvorsen.z = clamp(z + dt * dz, -15.0, 15.0);
}

/// Classify which of 3 Halvorsen lobes the state is in.
/// The dominant coordinate determines the lobe.
pub(crate) fn classify_halvorsen_lobe(halvorsen: &HalvorsenState) -> u8 {
    let ax = halvorsen.x.abs();
    let ay = halvorsen.y.abs();
    let az = halvorsen.z.abs();
    if ax >= ay && ax >= az {
        0
    } else if ay >= az {
        1
    } else {
        2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thomas_stays_bounded() {
        let mut t = ThomasState::default();
        for _ in 0..1000 {
            update_thomas(&mut t, 0.208, 0.05);
        }
        assert!(t.x.abs() <= 3.0);
        assert!(t.y.abs() <= 3.0);
        assert!(t.z.abs() <= 3.0);
    }

    #[test]
    fn test_aizawa_stays_bounded() {
        let mut a = AizawaState::default();
        for _ in 0..1000 {
            update_aizawa(&mut a, 0.01);
        }
        assert!(a.x.abs() <= 3.0);
        assert!(a.y.abs() <= 3.0);
        assert!(a.z.abs() <= 3.0);
    }

    #[test]
    fn test_halvorsen_stays_bounded() {
        let mut h = HalvorsenState::default();
        for _ in 0..1000 {
            update_halvorsen(&mut h, 0.01);
        }
        assert!(h.x.abs() <= 15.0);
        assert!(h.y.abs() <= 15.0);
        assert!(h.z.abs() <= 15.0);
    }

    #[test]
    fn test_thomas_basin_classification() {
        let t = ThomasState {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        };
        assert_eq!(classify_thomas_basin(&t), 0);

        let t2 = ThomasState {
            x: 1.0,
            y: 1.0,
            z: -1.0,
        };
        assert_eq!(classify_thomas_basin(&t2), 1);
    }

    #[test]
    fn test_halvorsen_lobe_classification() {
        let h = HalvorsenState {
            x: 5.0,
            y: 1.0,
            z: 1.0,
        };
        assert_eq!(classify_halvorsen_lobe(&h), 0);

        let h2 = HalvorsenState {
            x: 1.0,
            y: 5.0,
            z: 1.0,
        };
        assert_eq!(classify_halvorsen_lobe(&h2), 1);
    }
}
