(:rewrite-plan (:signal-min 0.62 :noise-max 0.38 :chaos-max 0.55)
 :logistic (:edge 3.56995 :distance-window 0.4 :aggression-base 0.35 :aggression-scale 0.65
            :aggression-min 0.05 :aggression-max 0.95)
 :lambdoma (:convergence-min 0.72)
 :lorenz (:sigma 10.0 :rho 28.0 :beta 2.6666667 :dt 0.01 :target-radius 25.0 :radius-window 25.0)
 :score (:base-weight 0.40
         :token-efficiency-weight 0.35
         :codemode-efficiency-weight 0.25
         :codemode-relay-budget 2.0
         :codemode-chain-weight 0.45
         :codemode-relay-weight 0.40
         :codemode-sources-weight 0.15)
 :complexity (:density-simple-mult 3.0 :density-possible-mult 8.0)
 :vitruvian (:strength-chaos-weight 0.6 :strength-bounded-weight 0.4
             :utility-global-weight 0.35 :utility-coherence-weight 0.25 :utility-balance-weight 0.20
             :utility-supervision-weight 0.20
             :beauty-ratio-weight 0.50 :beauty-inter-weight 0.25 :beauty-simplicity-weight 0.25
             :signal-strength-weight 0.34 :signal-utility-weight 0.33 :signal-beauty-weight 0.33)
 :memory (:bootstrap-skill-limit 3
          :bootstrap-skill-chars 1200
          :bootstrap-recent-limit 5
          :bootstrap-recent-chars 800
          :recall-limit 5
          :recall-max-chars 1500
          :crystal-min-score 0.7)
 :swarm (:evolution-decay 0.95
         :min-samples 3
         :score-weights (:harmony 0.3 :latency 0.2 :cost 0.2 :success-rate 0.3))
 :defaults (:required (:tui :openrouter-api-key)
            :optional (:s3 :telegram :slack :whatsapp :imessage :mqtt :tailscale :exa :brave))
 :security (:dissonance-weight 0.15
            :anomaly-threshold-stddev 2.0
            :digest-interval-hours 24
            :max-downgrades-per-hour 10
            :privileged-ops ("vault-set" "vault-delete" "config-set"
                            "harmony-policy-set" "matrix-set-edge"
                            "matrix-set-node" "matrix-reset-defaults"
                            "model-policy-upsert" "codemode-run"
                            "git-commit" "self-push")
            :admin-intent-required-for
             (:harmony-policy-set :matrix-set-edge :matrix-reset-defaults))
 :supervision (:taxonomy-default :auditable
               :confirmable-threshold 0.85
               :partial-threshold 0.60
               :auditable-threshold 0.40
               :window 64
               :evidence-timeout-ms 30000
               :test-timeout-ms 60000)
 :signalograd (:supervision (:rate-delta-max 0.06
                              :confidence-delta-max 0.05)
               :harmony (:signal-bias-max 0.06
                          :noise-bias-max 0.04
                          :rewrite-signal-delta-max 0.05
                          :rewrite-chaos-delta-max 0.04
                          :aggression-bias-max 0.08)
               :routing (:price-weight-delta-max 0.07
                          :speed-weight-delta-max 0.07
                          :success-weight-delta-max 0.05
                          :reasoning-weight-delta-max 0.06
                          :vitruvian-min-delta-max 0.04)
               :memory (:recall-limit-delta-max 2
                         :crystal-threshold-delta-max 0.05)
               :security (:dissonance-weight-delta-max 0.03
                           :anomaly-threshold-delta-max 0.25)
               :presentation (:verbosity-delta-max 0.22
                              :markdown-density-delta-max 0.18
                              :symbolic-density-delta-max 0.22
                              :self-reference-delta-max 0.22
                              :decor-density-delta-max 0.25)
               :reward (:noise-weight 0.6
                        :chaos-weight 0.4
                        :error-weight 0.3
                        :queue-weight 0.2
                        :max-errors 10.0
                        :max-queue-depth 10.0)
               :feedback (:reward-accept-min 0.58
                          :stability-accept-min 0.55
                          :user-affinity-accept-min 0.35
                          :cleanliness-accept-min 0.55
                          :recall-strength-hit-min 0.12)
               :telemetry (:error-consecutive-scale 6.0
                           :error-total-scale 24.0
                           :presentation-verbosity-reference-words 120.0)
               :audit (:detail-max-chars 320)
               :swarm (:latency-reference-ms 8000.0
                       :cost-scale 20.0)
               :stability (:chaos-weight 0.45
                           :ratio-weight 0.30
                           :bounded-weight 0.25)
               :novelty (:interdisciplinary-weight 0.6
                         :density-weight 0.4
                         :density-scale 8.0)
               :limits (:rewrite-signal-min 0.20
                        :rewrite-signal-max 0.95
                        :rewrite-chaos-min 0.05
                        :rewrite-chaos-max 0.95
                        :memory-crystal-min 0.10
                        :memory-crystal-max 0.98
                        :security-dissonance-min 0.05
                        :security-dissonance-max 0.95
                        :security-anomaly-min 0.50
                        :security-anomaly-max 4.0
                        :memory-recall-min 2
                        :memory-recall-max 12
                        :memory-bootstrap-min 1
                        :memory-bootstrap-max 8
                        :routing-weight-min 0.05
                        :routing-weight-max 0.70
                        :routing-vitruvian-min 0.30
                        :routing-vitruvian-max 0.95
                        :aggression-min 0.01
                        :aggression-max 0.99))
 :evolution (:mode :artifact-rollout
             :source-rewrite-enabled nil
             :distributed (:enabled nil
                           :store :s3
                           :quorum-min 0.66
                           :accept-threshold 0.75
                           :degrade-threshold 0.55))
 :distributed-evolution (:enabled nil
                        :store-kind :s3
                        :store-bucket ""
                        :store-prefix "harmonia/evolution"
                        :quorum-min 0.66
                        :accept-threshold 0.75
                        :degrade-threshold 0.55
                        :max-local-divergence 0.25
                        :rollback-on-dissonance-threshold 0.40))
