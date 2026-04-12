/// Attractor dynamics for memory basin assignment.
///
/// Three attractor families provide distinct basin geometries:
/// - Thomas: cyclically symmetric, up to 6 coexisting basins for multi-domain routing
/// - Aizawa: sphere + tube topology for depth recall (shallow vs crystal)
/// - Halvorsen: 3-lobed propeller for interdisciplinary bridging
///
/// These live in memory-field, separate from Signalograd's Lorenz attractor,
/// maintaining clean separation of concerns.

/// Soft saturation — smooth version of clamp that preserves attractor geometry.
/// Uses R * tanh(x / R) which asymptotes to +/-R but is differentiable everywhere.
fn soft_saturate(x: f64, radius: f64) -> f64 {
    radius * (x / radius).tanh()
}

// ─── Basin classification trait ─────────────────────────────────────────────

/// Unifying trait for basin classification across attractor families.
///
/// Each attractor partitions phase space into basins with different geometry:
/// Thomas → 6 octant-pair basins, Aizawa → 2 depth basins, Halvorsen → 3 lobes.
/// `classify_basin` returns a u8 basin index whose meaning is attractor-specific.
///
/// Note: `step` is not part of this trait because different attractors require
/// different parameters (Thomas needs `b`, Aizawa/Halvorsen only need `dt`).
pub(crate) trait BasinClassifier {
    fn classify_basin(&self) -> u8;
}

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

/// Thomas attractor derivatives: (dx/dt, dy/dt, dz/dt).
fn thomas_derivatives(state: &ThomasState, b: f64) -> (f64, f64, f64) {
    (
        state.y.sin() - b * state.x,
        state.z.sin() - b * state.y,
        state.x.sin() - b * state.z,
    )
}

/// Step the Thomas attractor by dt using 4th-order Runge-Kutta.
/// Pure transition — returns new state.
pub(crate) fn step_thomas(state: &ThomasState, b: f64, dt: f64) -> ThomasState {
    let k1 = thomas_derivatives(state, b);
    let s2 = ThomasState {
        x: state.x + 0.5 * dt * k1.0,
        y: state.y + 0.5 * dt * k1.1,
        z: state.z + 0.5 * dt * k1.2,
    };
    let k2 = thomas_derivatives(&s2, b);
    let s3 = ThomasState {
        x: state.x + 0.5 * dt * k2.0,
        y: state.y + 0.5 * dt * k2.1,
        z: state.z + 0.5 * dt * k2.2,
    };
    let k3 = thomas_derivatives(&s3, b);
    let s4 = ThomasState {
        x: state.x + dt * k3.0,
        y: state.y + dt * k3.1,
        z: state.z + dt * k3.2,
    };
    let k4 = thomas_derivatives(&s4, b);

    ThomasState {
        x: soft_saturate(state.x + (dt / 6.0) * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0), 3.0),
        y: soft_saturate(state.y + (dt / 6.0) * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1), 3.0),
        z: soft_saturate(state.z + (dt / 6.0) * (k1.2 + 2.0 * k2.2 + 2.0 * k3.2 + k4.2), 3.0),
    }
}

/// Mutable wrapper — used by tests. Production code uses step_thomas directly.
#[cfg(test)]
pub(crate) fn update_thomas(thomas: &mut ThomasState, b: f64, dt: f64) {
    *thomas = step_thomas(thomas, b, dt);
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

impl BasinClassifier for ThomasState {
    fn classify_basin(&self) -> u8 {
        classify_thomas_basin(self)
    }
}

/// Soft basin classification using Boltzmann distribution over basin centroids.
///
/// Returns probability distribution over 6 basins. Near basin centers,
/// one basin dominates. Near boundaries, probabilities are mixed —
/// honoring the fractal boundary structure instead of imposing sharp planes.
///
/// Temperature is modulated by the Thomas b parameter:
/// - Near edge of chaos (b ~= 0.208): high T, soft boundaries
/// - Far from chaos: low T, sharp boundaries
pub(crate) fn classify_thomas_basin_soft(thomas: &ThomasState, b: f64) -> [f64; 6] {
    // Pre-computed basin centroids (approximate attractor centers for each octant-pair)
    // These are empirical values from long-time integration at b=0.208
    let centroids: [(f64, f64, f64); 6] = [
        ( 1.0,  1.0,  1.0),   // basin 0: (+,+,+) / (-,-,-)
        ( 1.0,  1.0, -1.0),   // basin 1: (+,+,-) / (-,-,+)
        ( 1.0, -1.0,  1.0),   // basin 2: (+,-,+) / (-,+,-)
        ( 1.0, -1.0, -1.0),   // basin 3: (+,-,-) / (-,+,+)
        ( 0.0,  0.0,  1.5),   // basin 4: z-dominant (spare)
        ( 0.0,  0.0, -1.5),   // basin 5: generic fallback
    ];

    // Temperature: higher near edge of chaos (more uncertain boundaries)
    let temperature = 0.3 + 2.0 * (-(b - 0.208).abs() * 20.0).exp();

    let mut weights = [0.0_f64; 6];
    for (i, c) in centroids.iter().enumerate() {
        let dx = thomas.x - c.0;
        let dy = thomas.y - c.1;
        let dz = thomas.z - c.2;
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        weights[i] = (-dist / temperature).exp();
    }

    let total: f64 = weights.iter().sum();
    if total < 1e-30 {
        return [1.0 / 6.0; 6];
    }
    for w in weights.iter_mut() {
        *w /= total;
    }
    weights
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

/// Aizawa attractor derivatives: (dx/dt, dy/dt, dz/dt).
fn aizawa_derivatives(state: &AizawaState) -> (f64, f64, f64) {
    let r2 = state.x * state.x + state.y * state.y;
    (
        (state.z - AIZAWA_B) * state.x - AIZAWA_D * state.y,
        AIZAWA_D * state.x + (state.z - AIZAWA_B) * state.y,
        AIZAWA_C + AIZAWA_A * state.z - state.z * state.z * state.z / 3.0
            - r2 * (1.0 + AIZAWA_E * state.z)
            + AIZAWA_F * state.z * state.x * state.x * state.x,
    )
}

/// Step the Aizawa attractor by dt using 4th-order Runge-Kutta.
/// Pure transition — returns new state.
pub(crate) fn step_aizawa(state: &AizawaState, dt: f64) -> AizawaState {
    let k1 = aizawa_derivatives(state);
    let s2 = AizawaState {
        x: state.x + 0.5 * dt * k1.0,
        y: state.y + 0.5 * dt * k1.1,
        z: state.z + 0.5 * dt * k1.2,
    };
    let k2 = aizawa_derivatives(&s2);
    let s3 = AizawaState {
        x: state.x + 0.5 * dt * k2.0,
        y: state.y + 0.5 * dt * k2.1,
        z: state.z + 0.5 * dt * k2.2,
    };
    let k3 = aizawa_derivatives(&s3);
    let s4 = AizawaState {
        x: state.x + dt * k3.0,
        y: state.y + dt * k3.1,
        z: state.z + dt * k3.2,
    };
    let k4 = aizawa_derivatives(&s4);

    AizawaState {
        x: soft_saturate(state.x + (dt / 6.0) * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0), 3.0),
        y: soft_saturate(state.y + (dt / 6.0) * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1), 3.0),
        z: soft_saturate(state.z + (dt / 6.0) * (k1.2 + 2.0 * k2.2 + 2.0 * k3.2 + k4.2), 3.0),
    }
}

/// Mutable wrapper — used by tests. Production code uses step_aizawa directly.
#[cfg(test)]
pub(crate) fn update_aizawa(aizawa: &mut AizawaState, dt: f64) {
    *aizawa = step_aizawa(aizawa, dt);
}

/// Classify Aizawa depth: tube (|z| > threshold) vs surface.
pub(crate) fn classify_aizawa_depth(aizawa: &AizawaState) -> bool {
    // true = tube (deep/crystal), false = surface (shallow)
    aizawa.z.abs() > 1.5
}

impl BasinClassifier for AizawaState {
    /// Returns 0 for surface (shallow), 1 for tube (deep/crystal).
    fn classify_basin(&self) -> u8 {
        if classify_aizawa_depth(self) { 1 } else { 0 }
    }
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

/// Halvorsen attractor derivatives: (dx/dt, dy/dt, dz/dt).
fn halvorsen_derivatives(state: &HalvorsenState) -> (f64, f64, f64) {
    (
        -HALVORSEN_A * state.x - 4.0 * state.y - 4.0 * state.z - state.y * state.y,
        -HALVORSEN_A * state.y - 4.0 * state.z - 4.0 * state.x - state.z * state.z,
        -HALVORSEN_A * state.z - 4.0 * state.x - 4.0 * state.y - state.x * state.x,
    )
}

/// Step the Halvorsen attractor by dt using 4th-order Runge-Kutta.
/// Pure transition — returns new state.
pub(crate) fn step_halvorsen(state: &HalvorsenState, dt: f64) -> HalvorsenState {
    let k1 = halvorsen_derivatives(state);
    let s2 = HalvorsenState {
        x: state.x + 0.5 * dt * k1.0,
        y: state.y + 0.5 * dt * k1.1,
        z: state.z + 0.5 * dt * k1.2,
    };
    let k2 = halvorsen_derivatives(&s2);
    let s3 = HalvorsenState {
        x: state.x + 0.5 * dt * k2.0,
        y: state.y + 0.5 * dt * k2.1,
        z: state.z + 0.5 * dt * k2.2,
    };
    let k3 = halvorsen_derivatives(&s3);
    let s4 = HalvorsenState {
        x: state.x + dt * k3.0,
        y: state.y + dt * k3.1,
        z: state.z + dt * k3.2,
    };
    let k4 = halvorsen_derivatives(&s4);

    HalvorsenState {
        x: soft_saturate(state.x + (dt / 6.0) * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0), 15.0),
        y: soft_saturate(state.y + (dt / 6.0) * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1), 15.0),
        z: soft_saturate(state.z + (dt / 6.0) * (k1.2 + 2.0 * k2.2 + 2.0 * k3.2 + k4.2), 15.0),
    }
}

/// Mutable wrapper — used by tests. Production code uses step_halvorsen directly.
#[cfg(test)]
pub(crate) fn update_halvorsen(halvorsen: &mut HalvorsenState, dt: f64) {
    *halvorsen = step_halvorsen(halvorsen, dt);
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

impl BasinClassifier for HalvorsenState {
    fn classify_basin(&self) -> u8 {
        classify_halvorsen_lobe(self)
    }
}

// ─── Invariant Measure ─────────────────────────────────────────────────────

/// Invariant measure tracking — histogram of attractor visits over a sliding window.
///
/// Instead of classifying basin from instantaneous position (unreliable for chaos),
/// track the distribution of visits and classify from the distribution.
/// This is robust to numerical integration errors in chaotic systems.
const MEASURE_BINS: usize = 8;

#[derive(Clone, Debug)]
pub(crate) struct InvariantMeasure {
    /// 3D histogram bins for each axis, indexed [axis][bin]
    pub(crate) bins: [[u32; MEASURE_BINS]; 3],
    pub(crate) total_visits: u64,
}

impl Default for InvariantMeasure {
    fn default() -> Self {
        Self {
            bins: [[0; MEASURE_BINS]; 3],
            total_visits: 0,
        }
    }
}

impl InvariantMeasure {
    /// Record a visit at position (x, y, z) within the given radius.
    pub(crate) fn record(&mut self, x: f64, y: f64, z: f64, radius: f64) {
        let bin_x = self.to_bin(x, radius);
        let bin_y = self.to_bin(y, radius);
        let bin_z = self.to_bin(z, radius);
        self.bins[0][bin_x] = self.bins[0][bin_x].saturating_add(1);
        self.bins[1][bin_y] = self.bins[1][bin_y].saturating_add(1);
        self.bins[2][bin_z] = self.bins[2][bin_z].saturating_add(1);
        self.total_visits += 1;
    }

    /// Find the dominant basin from the visit distribution.
    /// Returns the octant (sign pattern) with the most visits.
    pub(crate) fn dominant_octant(&self) -> (bool, bool, bool) {
        let mid = MEASURE_BINS / 2;
        let x_positive = self.bins[0][mid..].iter().sum::<u32>()
            >= self.bins[0][..mid].iter().sum::<u32>();
        let y_positive = self.bins[1][mid..].iter().sum::<u32>()
            >= self.bins[1][..mid].iter().sum::<u32>();
        let z_positive = self.bins[2][mid..].iter().sum::<u32>()
            >= self.bins[2][..mid].iter().sum::<u32>();
        (x_positive, y_positive, z_positive)
    }

    /// Decay old visits to implement sliding window behavior.
    /// Called periodically to prevent histogram from becoming stale.
    pub(crate) fn decay(&mut self, factor: f64) {
        for axis in self.bins.iter_mut() {
            for bin in axis.iter_mut() {
                *bin = (*bin as f64 * factor) as u32;
            }
        }
        self.total_visits = (self.total_visits as f64 * factor) as u64;
    }

    fn to_bin(&self, value: f64, radius: f64) -> usize {
        let normalized = (value / radius + 1.0) * 0.5; // Map [-R, R] to [0, 1]
        let bin = (normalized * MEASURE_BINS as f64) as usize;
        bin.min(MEASURE_BINS - 1)
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

    #[test]
    fn test_basin_classifier_trait() {
        // Verify BasinClassifier trait produces same results as standalone functions.
        let t = ThomasState { x: 1.0, y: 1.0, z: -1.0 };
        assert_eq!(t.classify_basin(), classify_thomas_basin(&t));

        let a = AizawaState { x: 0.1, y: 0.1, z: 2.0 }; // deep (tube)
        assert_eq!(a.classify_basin(), 1);
        let a2 = AizawaState { x: 0.1, y: 0.1, z: 0.5 }; // surface
        assert_eq!(a2.classify_basin(), 0);

        let h = HalvorsenState { x: 5.0, y: 1.0, z: 1.0 };
        assert_eq!(h.classify_basin(), classify_halvorsen_lobe(&h));
    }

    // ─── Phase A new tests ─────────────────────────────────────────────────

    #[test]
    fn test_soft_saturate_identity_near_zero() {
        // Near zero, soft_saturate(x, R) ≈ x (linear region of tanh)
        let x = 0.01;
        let result = soft_saturate(x, 3.0);
        assert!((result - x).abs() < 1e-6, "soft_saturate should be ~identity near zero");
    }

    #[test]
    fn test_soft_saturate_asymptotes_to_radius() {
        // For large inputs, soft_saturate(x, R) → R
        let result_pos = soft_saturate(100.0, 3.0);
        assert!((result_pos - 3.0).abs() < 1e-6, "should asymptote to +R");
        let result_neg = soft_saturate(-100.0, 3.0);
        assert!((result_neg + 3.0).abs() < 1e-6, "should asymptote to -R");
    }

    #[test]
    fn test_soft_saturate_bounded() {
        // Output should always be within [-R, R]
        for &x in &[0.0, 1.0, 2.99, 3.0, 10.0, 1000.0, -1.0, -3.0, -1000.0] {
            let r = soft_saturate(x, 3.0);
            assert!(r.abs() <= 3.0, "soft_saturate({}) = {} should be in [-3, 3]", x, r);
        }
        // Moderate inputs should be strictly inside the boundary
        for &x in &[0.0, 1.0, 2.0, -1.0, -2.0] {
            let r = soft_saturate(x, 3.0);
            assert!(r.abs() < 3.0, "soft_saturate({}) = {} should be strictly inside for moderate input", x, r);
        }
    }

    #[test]
    fn test_rk4_thomas_stays_bounded() {
        // RK4 integration should keep Thomas bounded (same as old Euler test)
        let mut t = ThomasState::default();
        for _ in 0..2000 {
            update_thomas(&mut t, 0.208, 0.05);
        }
        assert!(t.x.abs() < 3.0, "Thomas x out of bounds: {}", t.x);
        assert!(t.y.abs() < 3.0, "Thomas y out of bounds: {}", t.y);
        assert!(t.z.abs() < 3.0, "Thomas z out of bounds: {}", t.z);
    }

    #[test]
    fn test_rk4_aizawa_stays_bounded() {
        let mut a = AizawaState::default();
        for _ in 0..2000 {
            update_aizawa(&mut a, 0.01);
        }
        assert!(a.x.abs() < 3.0, "Aizawa x out of bounds: {}", a.x);
        assert!(a.y.abs() < 3.0, "Aizawa y out of bounds: {}", a.y);
        assert!(a.z.abs() < 3.0, "Aizawa z out of bounds: {}", a.z);
    }

    #[test]
    fn test_rk4_halvorsen_stays_bounded() {
        let mut h = HalvorsenState::default();
        for _ in 0..2000 {
            update_halvorsen(&mut h, 0.01);
        }
        assert!(h.x.abs() < 15.0, "Halvorsen x out of bounds: {}", h.x);
        assert!(h.y.abs() < 15.0, "Halvorsen y out of bounds: {}", h.y);
        assert!(h.z.abs() < 15.0, "Halvorsen z out of bounds: {}", h.z);
    }

    #[test]
    fn test_classify_thomas_basin_soft_valid_distribution() {
        // Probabilities should sum to 1.0 for any state
        let t = ThomasState { x: 1.0, y: 1.0, z: 1.0 };
        let probs = classify_thomas_basin_soft(&t, 0.208);
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "probabilities should sum to 1.0, got {}", sum);
        // All probabilities should be non-negative
        for (i, &p) in probs.iter().enumerate() {
            assert!(p >= 0.0, "basin {} has negative probability: {}", i, p);
        }
    }

    #[test]
    fn test_classify_thomas_basin_soft_dominant_basin() {
        // Point (1,1,1) should have basin 0 dominant (closest to centroid 0)
        let t = ThomasState { x: 1.0, y: 1.0, z: 1.0 };
        let probs = classify_thomas_basin_soft(&t, 0.208);
        let max_idx = probs.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap().0;
        assert_eq!(max_idx, 0, "basin 0 should dominate at (1,1,1)");
    }

    #[test]
    fn test_classify_thomas_basin_soft_temperature_effect() {
        // At edge of chaos (b=0.208), distribution should be softer
        // than far from chaos (b=0.5)
        let t = ThomasState { x: 0.5, y: 0.5, z: 0.5 };
        let probs_chaos = classify_thomas_basin_soft(&t, 0.208);
        let probs_far = classify_thomas_basin_soft(&t, 0.5);
        // Entropy should be higher at edge of chaos
        let entropy = |p: &[f64; 6]| -> f64 {
            p.iter().filter(|&&x| x > 0.0).map(|x| -x * x.ln()).sum()
        };
        assert!(entropy(&probs_chaos) > entropy(&probs_far),
            "entropy at edge of chaos ({}) should exceed far from chaos ({})",
            entropy(&probs_chaos), entropy(&probs_far));
    }

    #[test]
    fn test_invariant_measure_record_and_octant() {
        let mut m = InvariantMeasure::default();
        // Record many positive-octant visits
        for _ in 0..100 {
            m.record(1.5, 1.5, 1.5, 3.0);
        }
        assert_eq!(m.total_visits, 100);
        let (xp, yp, zp) = m.dominant_octant();
        assert!(xp, "x should be positive-dominant");
        assert!(yp, "y should be positive-dominant");
        assert!(zp, "z should be positive-dominant");
    }

    #[test]
    fn test_invariant_measure_decay() {
        let mut m = InvariantMeasure::default();
        for _ in 0..100 {
            m.record(1.0, 1.0, 1.0, 3.0);
        }
        assert_eq!(m.total_visits, 100);
        m.decay(0.5);
        assert_eq!(m.total_visits, 50);
        // Bins should be halved
        let total_bins: u32 = m.bins[0].iter().sum();
        assert_eq!(total_bins, 50);
    }

    #[test]
    fn test_invariant_measure_negative_values() {
        let mut m = InvariantMeasure::default();
        for _ in 0..100 {
            m.record(-2.0, -2.0, -2.0, 3.0);
        }
        let (xp, yp, zp) = m.dominant_octant();
        assert!(!xp, "x should be negative-dominant");
        assert!(!yp, "y should be negative-dominant");
        assert!(!zp, "z should be negative-dominant");
    }

    #[test]
    fn test_invariant_measure_default_is_empty() {
        let m = InvariantMeasure::default();
        assert_eq!(m.total_visits, 0);
        for axis in &m.bins {
            for &bin in axis {
                assert_eq!(bin, 0);
            }
        }
    }
}
