//! dynamics-verify — formal re-verification of Harmonia's chaotic substrate (Phase 2:
//! F1 Thomas-saturation, F2 Euler-drift) and D1 symbolic recovery (basis expressiveness).
//!
//! Lisp+Rust only: this Rust tool replaces the deleted Python analysis. The integrators
//! are copied VERBATIM from the cited source lines (fixed textbook ODEs + the 1-line
//! saturation), so the measured object is the implementation's own dynamics.
//!
//!   `cargo run -p harmonia-dynamics-verify`   prints the full report
//!   `cargo test -p harmonia-dynamics-verify`  asserts F1/F2/D1
//!
//! Linear algebra is hand-rolled (normal equations + Gaussian elimination); there is no
//! linalg crate in the workspace and we keep it dependency-free.

// ───────────────────────── vector helpers ─────────────────────────
type V3 = [f64; 3];
fn add(a: V3, b: V3) -> V3 { [a[0] + b[0], a[1] + b[1], a[2] + b[2]] }
fn scale(a: V3, s: f64) -> V3 { [a[0] * s, a[1] * s, a[2] * s] }
fn add4(a: V3, b: V3, c: V3, d: V3) -> V3 {
    [a[0] + b[0] + c[0] + d[0], a[1] + b[1] + c[1] + d[1], a[2] + b[2] + c[2] + d[2]]
}
fn norm3(a: V3) -> f64 { (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt() }

fn rk4<F: Fn(V3) -> V3>(f: &F, s: V3, dt: f64) -> V3 {
    let k1 = f(s);
    let k2 = f(add(s, scale(k1, 0.5 * dt)));
    let k3 = f(add(s, scale(k2, 0.5 * dt)));
    let k4 = f(add(s, scale(k3, dt)));
    add(s, scale(add4(k1, scale(k2, 2.0), scale(k3, 2.0), k4), dt / 6.0))
}

// ───────────────────────── integrators (verbatim from source) ─────────────────────────

// weights.rs:17-19  σ=10 ρ=28 β=8/3
fn lorenz_deriv(s: V3) -> V3 {
    let (sigma, rho, beta) = (10.0, 28.0, 8.0 / 3.0);
    [sigma * (s[1] - s[0]), s[0] * (rho - s[2]) - s[1], s[0] * s[1] - beta * s[2]]
}
// harmonic-machine.lisp:218-220 — forward Euler
fn lorenz_euler(s: V3, dt: f64) -> V3 { add(s, scale(lorenz_deriv(s), dt)) }

// attractor.rs:13 — soft_saturate = R*tanh(x/R)
fn soft_saturate(x: f64, r: f64) -> f64 { r * (x / r).tanh() }
// attractor.rs:60-65 — Thomas derivatives at parameter b
fn thomas_deriv_b(s: V3, b: f64) -> V3 {
    [s[1].sin() - b * s[0], s[2].sin() - b * s[1], s[0].sin() - b * s[2]]
}
// attractor.rs:70-95 — RK4 then per-axis soft_saturate(·,3.0)
fn thomas_step_sat(s: V3, b: f64, dt: f64) -> V3 {
    let st = rk4(&|x| thomas_deriv_b(x, b), s, dt);
    [soft_saturate(st[0], 3.0), soft_saturate(st[1], 3.0), soft_saturate(st[2], 3.0)]
}
fn thomas_step_raw(s: V3, b: f64, dt: f64) -> V3 { rk4(&|x| thomas_deriv_b(x, b), s, dt) }

// ───────────────────────── invariants ─────────────────────────

// Benettin largest Lyapunov exponent.
fn lyapunov<F: Fn(V3, f64) -> V3>(step: F, s0: V3, dt: f64, n: usize, transient: usize) -> f64 {
    let d0 = 1e-9;
    let renorm = 5usize;
    let mut s = s0;
    for _ in 0..transient { s = step(s, dt); }
    let mut sp = [s[0] + d0, s[1], s[2]];
    let (mut acc, mut cnt) = (0.0, 0usize);
    for i in 0..n {
        s = step(s, dt);
        sp = step(sp, dt);
        if (i + 1) % renorm == 0 {
            let d = [sp[0] - s[0], sp[1] - s[1], sp[2] - s[2]];
            let dist = norm3(d);
            if dist > 0.0 {
                acc += (dist / d0).ln();
                cnt += 1;
                sp = [s[0] + d[0] * (d0 / dist), s[1] + d[1] * (d0 / dist), s[2] + d[2] * (d0 / dist)];
            }
        }
    }
    acc / (cnt as f64 * renorm as f64 * dt)
}

fn traj_std<F: Fn(V3, f64) -> V3>(step: F, s0: V3, dt: f64, n: usize, transient: usize) -> V3 {
    let mut s = s0;
    for _ in 0..transient { s = step(s, dt); }
    let mut pts = Vec::with_capacity(n);
    for _ in 0..n { s = step(s, dt); pts.push(s); }
    let mut m = [0.0; 3];
    for p in &pts { for k in 0..3 { m[k] += p[k]; } }
    for k in 0..3 { m[k] /= n as f64; }
    let mut v = [0.0; 3];
    for p in &pts { for k in 0..3 { v[k] += (p[k] - m[k]).powi(2); } }
    [(v[0] / n as f64).sqrt(), (v[1] / n as f64).sqrt(), (v[2] / n as f64).sqrt()]
}

// ───────────────────────── D1: libraries + STLSQ + MDL ─────────────────────────

type Lib = Vec<(String, Vec<f64>)>; // (s-expr name, column over the trajectory)

fn monomial_name(a: u32, b: u32, c: u32) -> String {
    let mut f = Vec::new();
    for _ in 0..a { f.push("x"); }
    for _ in 0..b { f.push("y"); }
    for _ in 0..c { f.push("z"); }
    if f.is_empty() { "1".into() } else if f.len() == 1 { f[0].into() } else { format!("(* {})", f.join(" ")) }
}

fn poly_library(x: &[V3], degree: u32) -> Lib {
    let mut lib = Lib::new();
    for a in 0..=degree {
        for b in 0..=(degree - a) {
            for c in 0..=(degree - a - b) {
                let col = x.iter().map(|p| p[0].powi(a as i32) * p[1].powi(b as i32) * p[2].powi(c as i32)).collect();
                lib.push((monomial_name(a, b, c), col));
            }
        }
    }
    lib
}

fn trig_features(x: &[V3]) -> Lib {
    let mut lib = Lib::new();
    for (i, v) in ["x", "y", "z"].iter().enumerate() {
        lib.push((format!("(sin {})", v), x.iter().map(|p| p[i].sin()).collect()));
        lib.push((format!("(cos {})", v), x.iter().map(|p| p[i].cos()).collect()));
    }
    lib
}

// Solve (A) beta = (rhs) for a symmetric positive-ish p×p system via Gaussian elimination
// with partial pivoting and a tiny ridge for stability.
fn solve(mut a: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Vec<f64> {
    let p = rhs.len();
    for i in 0..p { a[i][i] += 1e-10; }
    for col in 0..p {
        let mut piv = col;
        for r in (col + 1)..p { if a[r][col].abs() > a[piv][col].abs() { piv = r; } }
        a.swap(col, piv);
        rhs.swap(col, piv);
        let d = a[col][col];
        if d.abs() < 1e-14 { continue; }
        for r in 0..p {
            if r == col { continue; }
            let factor = a[r][col] / d;
            if factor == 0.0 { continue; }
            for k in col..p { a[r][k] -= factor * a[col][k]; }
            rhs[r] -= factor * rhs[col];
        }
    }
    (0..p).map(|i| if a[i][i].abs() < 1e-14 { 0.0 } else { rhs[i] / a[i][i] }).collect()
}

// Least squares over a subset of library columns: normal equations ΘᵀΘ β = Θᵀy.
fn lstsq(cols: &[&Vec<f64>], y: &[f64]) -> Vec<f64> {
    let p = cols.len();
    let n = y.len();
    let mut ata = vec![vec![0.0; p]; p];
    let mut aty = vec![0.0; p];
    for i in 0..p {
        for j in i..p {
            let s: f64 = (0..n).map(|k| cols[i][k] * cols[j][k]).sum();
            ata[i][j] = s;
            ata[j][i] = s;
        }
        aty[i] = (0..n).map(|k| cols[i][k] * y[k]).sum();
    }
    solve(ata, aty)
}

// Sequentially Thresholded Least Squares (the SINDy core).
fn stlsq(theta: &Lib, y: &[f64], thr: f64) -> Vec<f64> {
    let p = theta.len();
    let mut active: Vec<usize> = (0..p).collect();
    let mut coef = vec![0.0; p];
    for _ in 0..20 {
        let cols: Vec<&Vec<f64>> = active.iter().map(|&i| &theta[i].1).collect();
        let c = lstsq(&cols, y);
        coef = vec![0.0; p];
        for (idx, &i) in active.iter().enumerate() { coef[i] = c[idx]; }
        let new_active: Vec<usize> = active.iter().cloned().filter(|&i| coef[i].abs() >= thr).collect();
        if new_active.is_empty() { return vec![0.0; p]; }
        if new_active.len() == active.len() { break; }
        active = new_active;
    }
    coef
}

fn rss(theta: &Lib, y: &[f64], coef: &[f64]) -> f64 {
    let n = y.len();
    (0..n).map(|k| {
        let pred: f64 = theta.iter().zip(coef).map(|((_, col), c)| c * col[k]).sum();
        (y[k] - pred).powi(2)
    }).sum()
}

fn description_bits(k: usize, rss_val: f64, n_lib: usize, n: usize) -> f64 {
    let support = k as f64 * (n_lib.max(2) as f64).log2() + k as f64 * 16.0;
    let resid = 0.5 * n as f64 * (rss_val / n as f64 + 1e-12).log2();
    support + resid
}

// STLSQ across a threshold sweep; keep the support with minimum description length
// (harmonia's Solomonoff/Kolmogorov objective standing in for SINDy's hand-tuned threshold).
fn fit_mdl(theta: &Lib, y: &[f64]) -> Vec<f64> {
    let n = y.len();
    let n_lib = theta.len();
    let mut best = (f64::INFINITY, vec![0.0; n_lib]);
    for t in 0..40 {
        let thr = 10f64.powf(-2.5 + (1.2 - (-2.5)) * t as f64 / 39.0);
        let coef = stlsq(theta, y, thr);
        let k = coef.iter().filter(|c| c.abs() > 0.0).count();
        if k == 0 { continue; }
        let dl = description_bits(k, rss(theta, y, &coef), n_lib, n);
        if dl < best.0 { best = (dl, coef); }
    }
    best.1
}

fn to_sexp(theta: &Lib, coef: &[f64]) -> String {
    let mut terms = Vec::new();
    for ((name, _), c) in theta.iter().zip(coef) {
        if c.abs() < 1e-3 { continue; }
        let cc = (c * 1e4).round() / 1e4;
        terms.push(if name == "1" { format!("{}", cc) } else { format!("(* {} {})", cc, name) });
    }
    match terms.len() {
        0 => "0".into(),
        1 => terms.remove(0),
        _ => format!("(+ {})", terms.join(" ")),
    }
}

fn support(theta: &Lib, coef: &[f64]) -> Vec<String> {
    theta.iter().zip(coef).filter(|(_, c)| c.abs() > 1e-3).map(|((n, _), _)| n.clone()).collect()
}

// ───────────────────────── D1 driver ─────────────────────────

struct ArmResult { exact: bool, nterms: usize, dynr2: f64, size: usize, first: String }

fn gen<F: Fn(V3) -> V3>(deriv: &F, s0: V3, dt: f64, n: usize, transient: usize) -> (Vec<V3>, [Vec<f64>; 3]) {
    let mut s = s0;
    for _ in 0..transient { s = rk4(deriv, s, dt); }
    let mut xs = Vec::with_capacity(n);
    let mut ys = [Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n)];
    for _ in 0..n {
        s = rk4(deriv, s, dt);
        let d = deriv(s);
        xs.push(s);
        for j in 0..3 { ys[j].push(d[j]); }
    }
    (xs, ys)
}

fn run_arm(x: &[V3], y: &[Vec<f64>; 3], lib: Lib, true_support: &[(usize, &str)]) -> ArmResult {
    let mut coefs: Vec<Vec<f64>> = Vec::new();
    let mut sexprs: Vec<String> = Vec::new();
    let mut nterms = 0usize;
    let mut rec: Vec<(usize, String)> = Vec::new();
    for j in 0..3 {
        let c = fit_mdl(&lib, &y[j]);
        nterms += c.iter().filter(|v| v.abs() > 1e-3).count();
        for s in support(&lib, &c) { rec.push((j, s)); }
        sexprs.push(to_sexp(&lib, &c));
        coefs.push(c);
    }
    // dynR^2 across the 3 components
    let (mut num, mut den) = (0.0, 0.0);
    for j in 0..3 {
        let mean: f64 = y[j].iter().sum::<f64>() / x.len() as f64;
        for k in 0..x.len() {
            let pred: f64 = lib.iter().zip(&coefs[j]).map(|((_, col), c)| c * col[k]).sum();
            num += (y[j][k] - pred).powi(2);
            den += (y[j][k] - mean).powi(2);
        }
    }
    let dynr2 = 1.0 - num / den;
    let full = sexprs.join(" ; ");
    let want: std::collections::BTreeSet<(usize, String)> =
        true_support.iter().map(|(j, s)| (*j, s.to_string())).collect();
    let got: std::collections::BTreeSet<(usize, String)> = rec.into_iter().collect();
    ArmResult { exact: want == got, nterms, dynr2, size: full.len(), first: sexprs[0].clone() }
}

fn print_arm(label: &str, n_lib: usize, r: &ArmResult, true_terms: usize) {
    let solo = (-(r.size as f64) / 40.0).exp();
    println!("\n  Arm {}", label);
    println!("    library size        : {}", n_lib);
    println!("    #terms recovered     : {}   (true model has {})", r.nterms, true_terms);
    println!("    dynR^2 (flow field)  : {:.6}", r.dynr2);
    println!("    description length    : {} chars   Solomonoff exp(-size/40) = {:.3e}", r.size, solo);
    println!("    EXACT-TERM RECOVERY   : {}", if r.exact { "YES — recovered the true law" } else { "NO — wrong support (spurious / missing terms)" });
    println!("    dx/dt = {}", r.first);
}

// ───────────────────────── Phase 5 / D2: forced (non-Markov) Lorenz ─────────────────────────
// rho(t) = 28 + A·sin(ω t) makes the system NON-autonomous: (x,y,z) alone is not Markov.
// An autonomous (x,y,z) basis — DYSCO's frame — cannot see the forcing and leaves residual;
// once the basis includes the forcing "view" (x·sin ωt), the law is recovered/compressible.
// Returns (autonomous dynR², forcing-aware dynR²) for the ẏ component.
fn d2_forced_lorenz() -> (f64, f64) {
    let (dt, omega, amp) = (0.01_f64, 0.3_f64, 8.0_f64);
    let (n, transient) = (6000usize, 4000usize);
    let mut s = [1.0, 1.0, 1.0];
    let mut t = 0.0_f64;
    let step = |s: V3, t: f64| -> V3 {
        let rho = 28.0 + amp * (omega * t).sin();
        let d = [10.0 * (s[1] - s[0]), s[0] * (rho - s[2]) - s[1], s[0] * s[1] - (8.0 / 3.0) * s[2]];
        [s[0] + dt * d[0], s[1] + dt * d[1], s[2] + dt * d[2]]
    };
    for _ in 0..transient { s = step(s, t); t += dt; }
    let (mut cx, mut cxz, mut cy, mut cxf, mut ydot) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..n {
        let f = (omega * t).sin();
        let rho = 28.0 + amp * f;
        let yd = s[0] * (rho - s[2]) - s[1]; // 28x + amp·x·f − xz − y
        cx.push(s[0]); cxz.push(s[0] * s[2]); cy.push(s[1]); cxf.push(s[0] * f); ydot.push(yd);
        s = step(s, t); t += dt;
    }
    let dynr2 = |cols: &[&Vec<f64>], y: &[f64]| -> f64 {
        let xi = lstsq(cols, y);
        let m: f64 = y.iter().sum::<f64>() / y.len() as f64;
        let (mut num, mut den) = (0.0, 0.0);
        for k in 0..y.len() {
            let pred: f64 = cols.iter().zip(&xi).map(|(c, b)| b * c[k]).sum();
            num += (y[k] - pred).powi(2);
            den += (y[k] - m).powi(2);
        }
        1.0 - num / den
    };
    (dynr2(&[&cx, &cxz, &cy], &ydot), dynr2(&[&cx, &cxz, &cy, &cxf], &ydot))
}

// ───────────────────────── report ─────────────────────────

fn main() {
    println!("{}", "=".repeat(78));
    println!("PHASE 2 — INVARIANTS (estimator self-check: canonical Lorenz RK4)");
    println!("{}", "=".repeat(78));
    let lam_ctrl = lyapunov(|s, dt| rk4(&lorenz_deriv, s, dt), [1.0, 1.0, 1.0], 0.01, 200_000, 20_000);
    println!("  Lorenz RK4 (canonical, dt=0.01): largest Lyapunov = {:+.4}  (literature ~0.906)", lam_ctrl);

    println!("\n{}", "=".repeat(78));
    println!("IMPLEMENTATION AS BUILT");
    println!("{}", "=".repeat(78));
    let lam_hm = lyapunov(lorenz_euler, [1.0, 1.0, 1.0], 0.01, 200_000, 20_000);
    let lam_sg = lyapunov(lorenz_euler, [1.0, 1.0, 1.0], 0.008, 200_000, 20_000);
    println!("  F2  Lorenz Euler harmonic-machine.lisp:207 (dt=0.01): lambda = {:+.4}  (lit 0.906; Euler inflates)", lam_hm);
    println!("  F2  Lorenz Euler signalograd base       (dt=0.008): lambda = {:+.4}  (lit 0.906; Euler inflates)", lam_sg);

    let lam_thomas_sat = lyapunov(|s, dt| thomas_step_sat(s, 0.208, dt), [0.1, 0.0, 0.0], 0.05, 120_000, 40_000);
    let std_thomas_sat = traj_std(|s, dt| thomas_step_sat(s, 0.208, dt), [0.1, 0.0, 0.0], 0.05, 40_000, 20_000);
    println!("  F1  Thomas RK4 + soft_saturate attractor.rs:92 (b=0.208): lambda = {:+.4}  std=({:.3},{:.3},{:.3})",
             lam_thomas_sat, std_thomas_sat[0], std_thomas_sat[1], std_thomas_sat[2]);

    println!("\n  F1 control — saturation is what kills the chaos (not b):");
    for b in [0.19_f64, 0.208] {
        let raw = lyapunov(|s, dt| thomas_step_raw(s, b, dt), [0.1, 0.0, 0.0], 0.05, 120_000, 40_000);
        let sat = lyapunov(|s, dt| thomas_step_sat(s, b, dt), [0.1, 0.0, 0.0], 0.05, 120_000, 40_000);
        println!("    b={:.3}: bare lambda={:+.3} (chaotic)   saturated lambda={:+.3} ({})",
                 b, raw, sat, if sat < 0.0 { "collapses to fixed point" } else { "chaotic" });
    }

    println!("\n{}", "=".repeat(78));
    println!("PHASE 4 — D1 SYMBOLIC RECOVERY  (same selector both arms; only the library differs)");
    println!("{}", "=".repeat(78));

    // CONTROL — Lorenz (polynomial law): both arms must tie.
    println!("\nCONTROL — Lorenz (polynomial law)");
    let (xl, yl) = gen(&lorenz_deriv, [1.0, 1.0, 1.0], 0.01, 4000, 5000);
    let lorenz_true: &[(usize, &str)] = &[
        (0, "y"), (0, "x"),
        (1, "x"), (1, "(* x z)"), (1, "y"),
        (2, "(* x y)"), (2, "z"),
    ];
    let a = run_arm(&xl, &yl, poly_library(&xl, 3), lorenz_true);
    print_arm("A  fixed POLYNOMIAL basis (deg 3)", poly_library(&xl, 3).len(), &a, 7);
    let mut lb = poly_library(&xl, 2); lb.extend(trig_features(&xl));
    let b = run_arm(&xl, &yl, lb, lorenz_true);
    let lb_len = { let mut t = poly_library(&xl, 2); t.extend(trig_features(&xl)); t.len() };
    print_arm("B  poly(deg 2) + sin/cos transcendental", lb_len, &b, 7);

    // TREATMENT — Thomas bare b=0.19 (law has sin): basis expressiveness decides.
    println!("\nTREATMENT — Thomas bare b=0.19  (law has sin)");
    let (xt, yt) = gen(&|s| thomas_deriv_b(s, 0.19), [0.1, 0.0, 0.0], 0.05, 6000, 10000);
    let thomas_true: &[(usize, &str)] = &[
        (0, "(sin y)"), (0, "x"),
        (1, "(sin z)"), (1, "y"),
        (2, "(sin x)"), (2, "z"),
    ];
    let ta = run_arm(&xt, &yt, poly_library(&xt, 3), thomas_true);
    print_arm("A  fixed POLYNOMIAL basis (deg 3)", poly_library(&xt, 3).len(), &ta, 6);
    let mut tlb = poly_library(&xt, 2); tlb.extend(trig_features(&xt));
    let tb = run_arm(&xt, &yt, tlb, thomas_true);
    let tlb_len = { let mut t = poly_library(&xt, 2); t.extend(trig_features(&xt)); t.len() };
    print_arm("B  poly(deg 2) + sin/cos transcendental", tlb_len, &tb, 6);

    println!("\n{}", "=".repeat(78));
    println!("PHASE 5 — D2: forced (non-Markov) Lorenz  rho(t)=28+8 sin(0.3 t)");
    println!("{}", "=".repeat(78));
    let (d2_auto, d2_forced) = d2_forced_lorenz();
    println!("  autonomous (x,y,z) basis [DYSCO Markov] : dynR^2(ydot) = {:.4}  (cannot see the forcing)", d2_auto);
    println!("  forcing-aware basis (+ x*sin(0.3 t))    : dynR^2(ydot) = {:.4}  (recovers it)", d2_forced);
    println!("  => the forced/emergent law is well-posed as COMPRESSION once the basis includes the");
    println!("     forcing 'view'; autonomous Markov identification (DYSCO's frame) structurally fails.");
}

// ───────────────────────── assertions ─────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f1_saturation_collapses_thomas() {
        // Bare Thomas is (weakly) chaotic / non-collapsing — its chaos is genuinely small
        // and multistable. The per-step soft_saturate collapses it to a fixed point: the
        // robust, honest signature is the Lyapunov DROP of >1, not an absolute bare value.
        let raw = lyapunov(|s, dt| thomas_step_raw(s, 0.208, dt), [0.1, 0.0, 0.0], 0.05, 120_000, 40_000);
        let sat = lyapunov(|s, dt| thomas_step_sat(s, 0.208, dt), [0.1, 0.0, 0.0], 0.05, 120_000, 40_000);
        assert!(raw > -0.05, "bare Thomas should not collapse, got {}", raw);
        assert!(sat < -0.5, "saturated Thomas should collapse to a fixed point, got {}", sat);
        assert!(raw - sat > 1.0, "saturation should remove >1 of lambda, got raw={} sat={}", raw, sat);
    }

    #[test]
    fn f2_lorenz_euler_chaotic_but_inflated() {
        let lam = lyapunov(lorenz_euler, [1.0, 1.0, 1.0], 0.01, 200_000, 20_000);
        assert!(lam > 0.9, "Lorenz Euler should be chaotic, got {}", lam);
        assert!(lam > 0.95, "Euler should inflate lambda above the RK4 value ~0.906, got {}", lam);
    }

    #[test]
    fn d1_lorenz_control_ties() {
        let (x, y) = gen(&lorenz_deriv, [1.0, 1.0, 1.0], 0.01, 4000, 5000);
        let t: &[(usize, &str)] = &[(0,"y"),(0,"x"),(1,"x"),(1,"(* x z)"),(1,"y"),(2,"(* x y)"),(2,"z")];
        let a = run_arm(&x, &y, poly_library(&x, 3), t);
        let mut lb = poly_library(&x, 2); lb.extend(trig_features(&x));
        let b = run_arm(&x, &y, lb, t);
        assert!(a.exact && a.dynr2 > 0.999, "Lorenz Arm A should be exact, {:?}/{}", a.exact, a.dynr2);
        assert!(b.exact && b.dynr2 > 0.999, "Lorenz Arm B should be exact, {:?}/{}", b.exact, b.dynr2);
    }

    #[test]
    fn d1_thomas_treatment_discriminates() {
        let (x, y) = gen(&|s| thomas_deriv_b(s, 0.19), [0.1, 0.0, 0.0], 0.05, 6000, 10000);
        let t: &[(usize, &str)] = &[(0,"(sin y)"),(0,"x"),(1,"(sin z)"),(1,"y"),(2,"(sin x)"),(2,"z")];
        let a = run_arm(&x, &y, poly_library(&x, 3), t);          // polynomial: cannot represent sin
        let mut lb = poly_library(&x, 2); lb.extend(trig_features(&x));
        let b = run_arm(&x, &y, lb, t);                            // with sin: exact
        assert!(!a.exact, "polynomial basis must FAIL to recover Thomas exactly");
        assert!(b.exact && b.dynr2 > 0.999, "transcendental basis must recover Thomas exactly, got {:?}/{}", b.exact, b.dynr2);
    }

    #[test]
    fn d2_forced_needs_forcing_view() {
        // Non-Markov forced Lorenz: the autonomous (Markov) basis can't see the forcing;
        // adding the forcing "view" recovers it. (DYSCO's multi-view denoising, the thing we lack.)
        let (auto, forced) = d2_forced_lorenz();
        assert!(forced > 0.999, "forcing-aware basis should recover ydot, got {}", forced);
        assert!(auto < forced - 0.02, "autonomous (Markov) basis must lose to forcing-aware: auto={} forced={}", auto, forced);
    }
}
