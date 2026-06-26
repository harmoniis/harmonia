#!/usr/bin/env python3
"""D1 — symbolic recovery of governing equations (Phase 4, the centerpiece).

The honest discriminator is BASIS EXPRESSIVENESS (not gauge/L1):
  - Control  = Lorenz (polynomial law)  -> BOTH arms recover it exactly.
  - Treatment= Thomas (bare, b=0.19; law has sin) -> only the arm whose
    library contains transcendental primitives can recover it exactly.

  Arm A — fixed POLYNOMIAL basis (the DYSCO/SINDy family), STLSQ.
  Arm B — polynomial + TRANSCENDENTAL primitives (sin/cos), selected by a
          description-length (MDL) objective ~ harmonia's exp(-size/40) prior,
          emitting the recovered law as an s-expression.

We use exact analytic derivatives on clean trajectories so the result isolates
basis expressiveness (no derivative noise, no gauge) — exactly the controlled
comparison. F1's saturated map is out of scope; Thomas here is the BARE law.
"""
import numpy as np
from itertools import product

# ---------------- integrators (bare dynamics) ----------------

def lorenz_f(s, sigma=10.0, rho=28.0, beta=8.0/3.0):
    x, y, z = s
    return np.array([sigma*(y-x), x*(rho-z)-y, x*y-beta*z])

def thomas_f(s, b=0.19):
    x, y, z = s
    return np.array([np.sin(y)-b*x, np.sin(z)-b*y, np.sin(x)-b*z])

def rk4(f, s, dt):
    k1=f(s); k2=f(s+0.5*dt*k1); k3=f(s+0.5*dt*k2); k4=f(s+dt*k3)
    return s + (dt/6.0)*(k1+2*k2+2*k3+k4)

def trajectory(f, s0, dt, n, transient):
    s=np.array(s0,float)
    for _ in range(transient): s=rk4(f,s,dt)
    X=np.empty((n,3))
    for i in range(n):
        s=rk4(f,s,dt); X[i]=s
    return X

# ---------------- libraries ----------------

def monomial(exps):
    """exps=(a,b,c) -> (name_for_sexp, fn). '1' for the constant."""
    factors = ['x']*exps[0] + ['y']*exps[1] + ['z']*exps[2]
    if not factors: name = "1"
    elif len(factors)==1: name = factors[0]
    else: name = "(* " + " ".join(factors) + ")"
    def fn(X, e=exps):
        return (X[:,0]**e[0])*(X[:,1]**e[1])*(X[:,2]**e[2])
    return name, fn

def poly_library(degree):
    lib=[]
    for a,b,c in product(range(degree+1), repeat=3):
        if a+b+c<=degree:
            lib.append(monomial((a,b,c)))
    return lib

def trig_features():
    lib=[]
    for i,v in enumerate("xyz"):
        lib.append((f"(sin {v})", lambda X,i=i: np.sin(X[:,i])))
        lib.append((f"(cos {v})", lambda X,i=i: np.cos(X[:,i])))
    return lib

def design(X, lib):
    return np.column_stack([fn(X) for _,fn in lib])

# ---------------- regression ----------------

def description_bits(k, rss_val, n_lib, n):
    """MDL: bits to name the support + bits to encode the residual (Gaussian code)."""
    support_bits = k*np.log2(max(2, n_lib)) + k*16   # which terms + ~16 bits/coef
    resid_bits = 0.5*n*np.log2(rss_val/n + 1e-12)
    return support_bits + resid_bits

def stlsq(Theta, y, thr, iters=20):
    """Sequentially Thresholded Least Squares (the SINDy core): start from the full
    least-squares fit, zero coefficients below `thr`, refit on survivors, repeat.
    Robust to collinear dictionaries where greedy/OMP fails."""
    xi = np.linalg.lstsq(Theta, y, rcond=None)[0]
    for _ in range(iters):
        small = np.abs(xi) < thr
        if small.all(): break
        xi[small] = 0.0; big = ~small
        xi[big] = np.linalg.lstsq(Theta[:, big], y, rcond=None)[0]
    return xi

def fit_mdl(Theta, y, n_lib):
    """STLSQ across a threshold sweep; keep the support with minimum description
    length — harmonia's Solomonoff/Kolmogorov objective standing in for SINDy's
    hand-tuned threshold. The SAME selector is used for BOTH arms, so the only
    experimental variable is the library (= basis expressiveness)."""
    n = len(y); best = (np.inf, np.zeros(Theta.shape[1]))
    for thr in np.logspace(-2.5, 1.2, 40):
        xi = stlsq(Theta, y, thr)
        k = int(np.count_nonzero(xi))
        if k == 0: continue
        r = float(np.sum((y - Theta @ xi) ** 2))
        dl = description_bits(k, r, n_lib, n)
        if dl < best[0]: best = (dl, xi.copy())
    return best[1]

# ---------------- s-expr + metrics ----------------

def to_sexp(xi, lib, tol=1e-3):
    terms=[]
    for c,(name,_) in zip(xi, lib):
        if abs(c)<tol: continue
        cc = round(float(c),4)
        terms.append(f"{cc}" if name=="1" else f"(* {cc} {name})")
    if not terms: return "0"
    if len(terms)==1: return terms[0]
    return "(+ " + " ".join(terms) + ")"

def dynR2(Theta_list, Y, XIs):
    num=0.0; den=0.0
    for j in range(3):
        pred=Theta_list[j]@XIs[j]
        num+=np.sum((Y[:,j]-pred)**2)
        den+=np.sum((Y[:,j]-Y[:,j].mean())**2)
    return 1.0 - num/den

def run(name, f, s0, dt, n, transient, true_support):
    X = trajectory(f, s0, dt, n, transient)
    Y = np.array([f(x) for x in X])   # (N, 3): rows = dx/dt at each sample
    print(f"\n{'='*78}\n{name}   (N={n} samples on the attractor)\n{'='*78}")
    for arm, lib in (("A  fixed POLYNOMIAL basis (deg 3)", poly_library(3)),
                     ("B  poly(deg 2) + sin/cos transcendental", poly_library(2)+trig_features())):
        Theta = design(X, lib); nlib=len(lib)
        Theta_list=[Theta,Theta,Theta]; XIs=[]; sexprs=[]; nterms=0
        for j in range(3):
            xi = fit_mdl(Theta, Y[:,j], nlib)   # SAME selector (STLSQ + MDL) for both arms
            XIs.append(xi); nterms+=int(np.count_nonzero(xi))
            sexprs.append(to_sexp(xi, lib))
        r2 = dynR2(Theta_list, Y, XIs)
        full = " ; ".join(sexprs)
        size = len(full)
        solo = np.exp(-size/40.0)
        # exact-term recovery: does the recovered support match the true one?
        rec_support = set()
        for j in range(3):
            for (nm,_),c in zip(lib, XIs[j]):
                if abs(c)>1e-3: rec_support.add((j,nm))
        exact = rec_support == true_support
        print(f"\n  Arm {arm}")
        print(f"    library size       : {nlib}")
        print(f"    #terms recovered    : {nterms}   (true model has {len(true_support)})")
        print(f"    dynR^2 (flow field) : {r2:.6f}")
        print(f"    description length   : {size} chars   Solomonoff exp(-size/40) = {solo:.3e}")
        print(f"    EXACT-TERM RECOVERY  : {'YES — recovered the true law' if exact else 'NO — wrong support (spurious / missing terms)'}")
        print(f"    dx/dt = {sexprs[0]}")

if __name__ == "__main__":
    np.seterr(all="ignore")
    # true supports keyed by (component, sexp-name)
    lorenz_support = {(0,"x"),(0,"y"), (1,"x"),(1,"(* x z)"),(1,"y"), (2,"(* x y)"),(2,"z")}
    thomas_support = {(0,"(sin y)"),(0,"x"), (1,"(sin z)"),(1,"y"), (2,"(sin x)"),(2,"z")}
    run("CONTROL — Lorenz (polynomial law; both arms should tie)",
        lorenz_f, [1.0,1.0,1.0], 0.01, 4000, 5000, lorenz_support)
    run("TREATMENT — Thomas bare b=0.19 (law has sin; basis expressiveness decides)",
        thomas_f, [0.1,0.0,0.0], 0.05, 6000, 10000, thomas_support)
