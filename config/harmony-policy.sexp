(:rewrite-plan (:signal-min 0.62 :noise-max 0.38 :chaos-max 0.55)
 :logistic (:edge 3.56995 :distance-window 0.4 :aggression-base 0.35 :aggression-scale 0.65
            :aggression-min 0.05 :aggression-max 0.95)
 :lambdoma (:convergence-min 0.72)
 :lorenz (:sigma 10.0 :rho 28.0 :beta 2.6666667 :dt 0.01 :target-radius 25.0 :radius-window 25.0)
 :complexity (:density-simple-mult 3.0 :density-possible-mult 8.0)
 :vitruvian (:strength-chaos-weight 0.6 :strength-bounded-weight 0.4
             :utility-global-weight 0.45 :utility-coherence-weight 0.30 :utility-balance-weight 0.25
             :beauty-ratio-weight 0.50 :beauty-inter-weight 0.25 :beauty-simplicity-weight 0.25
             :signal-strength-weight 0.34 :signal-utility-weight 0.33 :signal-beauty-weight 0.33))
