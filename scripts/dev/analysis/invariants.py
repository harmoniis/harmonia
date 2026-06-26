#!/usr/bin/env python3
"""Formal invariant re-verification for harmonia's chaotic substrate (Phase 2).

We re-implement the EXACT integrators read from source and measure the
dynamical-systems invariants that define each attractor, comparing to the
literature. This is the "pure scientific" retest: numbers, not assertions.

Integrators (verbatim from source):
  - Lorenz, harmonic machine:  harmonic-machine.lisp:207  (Euler, dt=0.01, sigma=10 rho=28 beta=8/3)
  - Lorenz, signalograd base:  kernel.rs:29 + weights.rs:17  (Euler, dt_base=0.008, telemetry=0 -> canonical)
  - Thomas, memory-field:      attractor.rs:60-95  (RK4 + soft_saturate(.,3.0), b=0.208, dt=0.05 default)

Estimators:
  - largest Lyapunov exponent: Benettin two-trajectory renormalization
  - correlation dimension:     Grassberger-Procaccia
The estimator is self-checked on canonical RK4 Lorenz (expect lambda~0.906, D2~2.05)
BEFORE we trust it on the implementation's actual (Euler) integrators.
"""
import numpy as np

# ----------------------------------------------------------------------------
# Integrators — copied verbatim from the harmonia source.
# ----------------------------------------------------------------------------

def lorenz_deriv(s, sigma=10.0, rho=28.0, beta=8.0/3.0):
    x, y, z = s
    return np.array([sigma*(y-x), x*(rho-z)-y, x*y-beta*z])

def lorenz_euler_step(s, dt, **p):
    # harmonic-machine.lisp:207  x2 = x + dt*dx  (forward Euler)
    return s + dt*lorenz_deriv(s, **p)

def lorenz_rk4_step(s, dt, **p):
    k1 = lorenz_deriv(s, **p)
    k2 = lorenz_deriv(s+0.5*dt*k1, **p)
    k3 = lorenz_deriv(s+0.5*dt*k2, **p)
    k4 = lorenz_deriv(s+dt*k3, **p)
    return s + (dt/6.0)*(k1+2*k2+2*k3+k4)

def soft_saturate(v, R=3.0):
    # attractor.rs:13  R * tanh(x/R)
    return R*np.tanh(v/R)

def thomas_deriv(s, b=0.208):
    x, y, z = s
    return np.array([np.sin(y)-b*x, np.sin(z)-b*y, np.sin(x)-b*z])

def thomas_rk4_nosat_step(s, dt, b=0.208):
    k1 = thomas_deriv(s, b)
    k2 = thomas_deriv(s+0.5*dt*k1, b)
    k3 = thomas_deriv(s+0.5*dt*k2, b)
    k4 = thomas_deriv(s+dt*k3, b)
    return s + (dt/6.0)*(k1+2*k2+2*k3+k4)

def thomas_rk4_step(s, dt, b=0.208):
    return soft_saturate(thomas_rk4_nosat_step(s, dt, b), 3.0)   # attractor.rs:92 — saturation IS part of the map

# ----------------------------------------------------------------------------
# Estimators
# ----------------------------------------------------------------------------

def largest_lyapunov(step, s0, dt, n=200000, transient=20000, d0=1e-9, renorm=5):
    """Benettin: track a nearby trajectory, renormalize every `renorm` steps."""
    s = np.array(s0, float)
    for _ in range(transient):
        s = step(s, dt)
    sp = s + d0*np.array([1.0, 0.0, 0.0])
    acc, cnt = 0.0, 0
    for i in range(n):
        s = step(s, dt); sp = step(sp, dt)
        if (i+1) % renorm == 0:
            d = sp - s
            dist = np.linalg.norm(d)
            if dist > 0:
                acc += np.log(dist/d0); cnt += 1
                sp = s + d*(d0/dist)
    return acc / (cnt*renorm*dt)

def correlation_dimension(step, s0, dt, n=4000, transient=20000, sample_every=20):
    """Grassberger-Procaccia: slope of log C(r) vs log r in the scaling band."""
    s = np.array(s0, float)
    for _ in range(transient):
        s = step(s, dt)
    pts = []
    for i in range(n*sample_every):
        s = step(s, dt)
        if i % sample_every == 0:
            pts.append(s.copy())
    P = np.array(pts)
    # pairwise distances (subsample for O(M^2) tractability)
    M = len(P)
    from itertools import combinations
    idx = np.random.default_rng(0).choice(M, size=min(M, 1500), replace=False)
    Q = P[idx]
    dists = []
    for a in range(len(Q)):
        dd = np.linalg.norm(Q[a+1:]-Q[a], axis=1)
        dists.append(dd)
    dists = np.concatenate(dists)
    dists = dists[dists > 0]
    if dists.size < 50:        # degenerate (collapsed to a point / limit set too small)
        return float('nan')
    rs = np.logspace(np.log10(dists.min()*2), np.log10(dists.max()/2), 24)
    C = np.array([(dists < r).mean() for r in rs])
    mask = (C > 1e-3) & (C < 0.5)
    if mask.sum() < 3:
        return float('nan')
    slope = np.polyfit(np.log(rs[mask]), np.log(C[mask]), 1)[0]
    return slope

def bounded(step, s0, dt, n=100000, bound=1e4):
    s = np.array(s0, float)
    for _ in range(n):
        s = step(s, dt)
        if not np.all(np.isfinite(s)) or np.max(np.abs(s)) > bound:
            return False
    return True

# ----------------------------------------------------------------------------
# Run
# ----------------------------------------------------------------------------

def traj_std(step, s0, dt, n=40000, transient=20000):
    s = np.array(s0, float)
    for _ in range(transient):
        s = step(s, dt)
    pts = []
    for _ in range(n):
        s = step(s, dt); pts.append(s.copy())
    return np.std(np.array(pts), axis=0)

def report(name, step, s0, dt, lit_lam, lit_d2, **kw):
    f = lambda s, d: step(s, d, **kw)
    sd = traj_std(f, s0, dt)
    bnd = bounded(f, s0, dt)
    lam = largest_lyapunov(f, s0, dt)
    print(f"\n{name}")
    print(f"  dt={dt}  bounded={bnd}  traj_std(x,y,z)=({sd[0]:.3f}, {sd[1]:.3f}, {sd[2]:.3f})")
    print(f"  largest Lyapunov  = {lam:+.4f}   (literature {lit_lam})")
    d2 = correlation_dimension(f, s0, dt)
    print(f"  correlation dim   = {d2:.3f}     (literature {lit_d2})")
    return lam, d2, bnd

if __name__ == "__main__":
    np.seterr(all="ignore")
    s0 = [0.1, 0.0, 0.0]
    print("="*70)
    print("ESTIMATOR SELF-CHECK — canonical Lorenz RK4 (must match literature)")
    print("="*70)
    report("Lorenz RK4 (canonical, dt=0.01)", lorenz_rk4_step, s0, 0.01, "~0.906", "~2.05")
    print("\n" + "="*70)
    print("IMPLEMENTATION AS BUILT")
    print("="*70)
    report("Lorenz Euler — harmonic-machine.lisp:207 (dt=0.01)", lorenz_euler_step, s0, 0.01, "~0.906", "~2.05")
    report("Lorenz Euler — signalograd base kernel.rs (dt=0.008)", lorenz_euler_step, s0, 0.008, "~0.906", "~2.05")
    report("Thomas RK4 + soft_saturate — attractor.rs (b=0.208, dt=0.05)", thomas_rk4_step, s0, 0.05, ">0 (chaotic)", "~1.8-2.0", b=0.208)

    print("\n" + "="*70)
    print("THOMAS REGIME CONTROL — is b=0.208 genuinely non-chaotic, or an artifact?")
    print("  (b=0.19 is the known-chaotic control; must show lam>0 to trust the integrator)")
    print("="*70)
    ics = [[0.1, 0.0, 0.0], [1.0, 1.0, 1.0], [-0.5, 0.3, 0.8]]
    for b in (0.19, 0.208):
        for sat, stepper in (("sat", thomas_rk4_step), ("raw", thomas_rk4_nosat_step)):
            lams = []
            for ic in ics:
                f = lambda s, d: stepper(s, d, b=b)
                lams.append(largest_lyapunov(f, ic, 0.05, n=120000, transient=40000))
            sd = traj_std(lambda s, d: stepper(s, d, b=b), ics[0], 0.05)
            tag = "CHAOTIC" if max(lams) > 0.02 else "non-chaotic (fixed pt / cycle)"
            print(f"  b={b} {sat}: lam over 3 ICs = [{lams[0]:+.3f},{lams[1]:+.3f},{lams[2]:+.3f}]  std0=({sd[0]:.2f},{sd[1]:.2f},{sd[2]:.2f})  -> {tag}")
