;;; signalograd.lisp — Lisp reflection layer for the Signalograd adaptive kernel.

(in-package :harmonia)

(defvar *model-evolution-policy* '())
(defvar *evolution-latest-dir* nil)
(defvar *evolution-versions-dir* nil)

(declaim (ftype function evolution-current-version))

(defun %signalograd-clamp (x lo hi)
  (max lo (min hi x)))

(defun %signalograd-policy-number (path default)
  (harmony-policy-number (format nil "signalograd/~A" path) default))

(defun %signalograd-policy-symmetric-clamp (value path default)
  (let ((limit (abs (%signalograd-policy-number path default))))
    (%signalograd-clamp value (- limit) limit)))

(defun %signalograd-policy-range-clamp (value min-path min-default max-path max-default)
  (%signalograd-clamp value
                      (%signalograd-policy-number min-path min-default)
                      (%signalograd-policy-number max-path max-default)))

(defun %signalograd-projection (&optional (runtime *runtime*))
  (or (and runtime (runtime-state-signalograd-projection runtime)) '()))

(defun signalograd-current-projection (&optional (runtime *runtime*))
  (%signalograd-projection runtime))

(defun %signalograd-section (name &optional (runtime *runtime*))
  (getf (%signalograd-projection runtime) name))

(defun %signalograd-section-value (section key default &optional (runtime *runtime*))
  (let ((node (%signalograd-section section runtime)))
    (if (and (listp node) (getf node key))
        (getf node key)
        default)))

(defun %signalograd-sexp (tag &rest plist)
  (prin1-to-string (cons tag plist)))

(defun %signalograd-detail-string (detail)
  (let* ((raw (cond
                ((null detail) "")
                ((stringp detail) detail)
                (t (prin1-to-string detail))))
         (limit (truncate (max 32 (%signalograd-policy-number "audit/detail-max-chars" 320.0)))))
    (if (> (length raw) limit)
        (subseq raw 0 limit)
        raw)))

(defun %signalograd-record-event (event-type &key cycle confidence stability novelty reward
                                              accepted recall-hits checkpoint-path
                                              checkpoint-digest detail)
  (when (fboundp 'chronicle-record-signalograd-event)
    (ignore-errors
      (chronicle-record-signalograd-event
       event-type
       :cycle (or cycle 0)
       :confidence (or confidence 0.0)
       :stability (or stability 0.0)
       :novelty (or novelty 0.0)
       :reward (or reward 0.0)
       :accepted accepted
       :recall-hits (or recall-hits 0)
       :checkpoint-path checkpoint-path
       :checkpoint-digest checkpoint-digest
       :detail (%signalograd-detail-string detail)))))

(defun %signalograd-latest-checkpoint-path ()
  (when (and (boundp '*evolution-latest-dir*) *evolution-latest-dir*)
    (merge-pathnames "signalograd.sexp" *evolution-latest-dir*)))

(defun %signalograd-version-checkpoint-path (&optional (version (and (fboundp 'evolution-current-version)
                                                                     (evolution-current-version))))
  (when (and version
             (> version 0)
             (boundp '*evolution-versions-dir*)
             *evolution-versions-dir*)
    (merge-pathnames "signalograd.sexp"
                     (merge-pathnames (format nil "v~D/" version) *evolution-versions-dir*))))

(defun %signalograd-sanitize-proposal (proposal)
  (let ((harmony (if (listp (getf proposal :harmony)) (getf proposal :harmony) '()))
        (routing (if (listp (getf proposal :routing)) (getf proposal :routing) '()))
        (memory (if (listp (getf proposal :memory)) (getf proposal :memory) '()))
        (security (if (listp (getf proposal :security-shell)) (getf proposal :security-shell) '()))
        (presentation (if (listp (getf proposal :presentation)) (getf proposal :presentation) '())))
    (list
     :cycle (or (getf proposal :cycle) 0)
     :confidence (%signalograd-clamp (or (getf proposal :confidence) 0.0) 0.0 1.0)
     :stability (%signalograd-clamp (or (getf proposal :stability) 0.0) 0.0 1.0)
     :novelty (%signalograd-clamp (or (getf proposal :novelty) 0.0) 0.0 1.0)
     :latent-energy (%signalograd-clamp (or (getf proposal :latent-energy) 0.0) 0.0 1.0)
     :recall-strength (%signalograd-clamp (or (getf proposal :recall-strength) 0.0) 0.0 1.0)
     :harmony
     (list :signal-bias (%signalograd-policy-symmetric-clamp
                         (or (getf harmony :signal-bias) 0.0)
                         "harmony/signal-bias-max" 0.06)
           :noise-bias (%signalograd-policy-symmetric-clamp
                        (or (getf harmony :noise-bias) 0.0)
                        "harmony/noise-bias-max" 0.04)
           :rewrite-signal-delta (%signalograd-policy-symmetric-clamp
                                  (or (getf harmony :rewrite-signal-delta) 0.0)
                                  "harmony/rewrite-signal-delta-max" 0.05)
           :rewrite-chaos-delta (%signalograd-policy-symmetric-clamp
                                 (or (getf harmony :rewrite-chaos-delta) 0.0)
                                 "harmony/rewrite-chaos-delta-max" 0.04)
           :aggression-bias (%signalograd-policy-symmetric-clamp
                             (or (getf harmony :aggression-bias) 0.0)
                             "harmony/aggression-bias-max" 0.08))
     :routing
     (list :price-weight-delta (%signalograd-policy-symmetric-clamp
                                (or (getf routing :price-weight-delta) 0.0)
                                "routing/price-weight-delta-max" 0.07)
           :speed-weight-delta (%signalograd-policy-symmetric-clamp
                                (or (getf routing :speed-weight-delta) 0.0)
                                "routing/speed-weight-delta-max" 0.07)
           :success-weight-delta (%signalograd-policy-symmetric-clamp
                                  (or (getf routing :success-weight-delta) 0.0)
                                  "routing/success-weight-delta-max" 0.05)
           :reasoning-weight-delta (%signalograd-policy-symmetric-clamp
                                    (or (getf routing :reasoning-weight-delta) 0.0)
                                    "routing/reasoning-weight-delta-max" 0.06)
           :vitruvian-min-delta (%signalograd-policy-symmetric-clamp
                                 (or (getf routing :vitruvian-min-delta) 0.0)
                                 "routing/vitruvian-min-delta-max" 0.04))
     :memory
     (list :recall-limit-delta (round (%signalograd-policy-symmetric-clamp
                                       (float (or (getf memory :recall-limit-delta) 0))
                                       "memory/recall-limit-delta-max" 2.0))
           :crystal-threshold-delta (%signalograd-policy-symmetric-clamp
                                     (or (getf memory :crystal-threshold-delta) 0.0)
                                     "memory/crystal-threshold-delta-max" 0.05))
     :security-shell
     (list :dissonance-weight-delta (%signalograd-policy-symmetric-clamp
                                     (or (getf security :dissonance-weight-delta) 0.0)
                                     "security/dissonance-weight-delta-max" 0.03)
           :anomaly-threshold-delta (%signalograd-policy-symmetric-clamp
                                     (or (getf security :anomaly-threshold-delta) 0.0)
                                     "security/anomaly-threshold-delta-max" 0.25))
     :presentation
     (list :verbosity-delta (%signalograd-policy-symmetric-clamp
                             (or (getf presentation :verbosity-delta) 0.0)
                             "presentation/verbosity-delta-max" 0.22)
           :markdown-density-delta (%signalograd-policy-symmetric-clamp
                                    (or (getf presentation :markdown-density-delta) 0.0)
                                    "presentation/markdown-density-delta-max" 0.18)
           :symbolic-density-delta (%signalograd-policy-symmetric-clamp
                                    (or (getf presentation :symbolic-density-delta) 0.0)
                                    "presentation/symbolic-density-delta-max" 0.22)
           :self-reference-delta (%signalograd-policy-symmetric-clamp
                                  (or (getf presentation :self-reference-delta) 0.0)
                                  "presentation/self-reference-delta-max" 0.22)
           :decor-density-delta (%signalograd-policy-symmetric-clamp
                                 (or (getf presentation :decor-density-delta) 0.0)
                                 "presentation/decor-density-delta-max" 0.25)))))

(defun signalograd-apply-proposal (proposal &key (runtime *runtime*))
  "Accept a proposal emitted by the Signalograd actor and make it effective for the next cycle."
  (when (and (listp proposal) (eq (first proposal) :signalograd-proposal))
    (setf proposal (rest proposal)))
  (setf proposal (%signalograd-sanitize-proposal proposal))
  (when runtime
    (setf (runtime-state-signalograd-projection runtime) proposal)
    (setf (runtime-state-signalograd-last-updated-at runtime) (get-universal-time))
    (when (%trace-level-p :standard)
      (trace-event "signalograd-proposal" :chain
                   :metadata (list :cycle (getf proposal :cycle)
                                   :confidence (getf proposal :confidence)
                                   :stability (getf proposal :stability)
                                   :novelty (getf proposal :novelty)
                                   :accepted t)))
    (runtime-log runtime :signalograd-projection
                 (list :confidence (getf proposal :confidence)
                       :stability (getf proposal :stability)
                       :recall-strength (getf proposal :recall-strength)
                       :cycle (getf proposal :cycle)))
    (%signalograd-record-event
     "proposal"
     :cycle (getf proposal :cycle)
     :confidence (getf proposal :confidence)
     :stability (getf proposal :stability)
     :novelty (getf proposal :novelty)
     :recall-hits (if (> (or (getf proposal :recall-strength) 0.0) 0.12) 1 0)
     :detail (list :harmony (getf proposal :harmony)
                   :routing (getf proposal :routing)
                   :memory (getf proposal :memory)
                   :security-shell (getf proposal :security-shell)
                   :presentation (getf proposal :presentation))))
  proposal)

(defun %signalograd-graph-stats (map)
  (let* ((nodes (length (getf map :concept-nodes)))
         (edges (length (getf map :concept-edges)))
         (inter 0))
    (dolist (edge (getf map :concept-edges))
      (when (getf edge :interdisciplinary)
        (incf inter)))
    (list :density (if (> nodes 0)
                       (/ edges (float (max 1 (* nodes nodes))))
                       0.0)
          :interdisciplinary (if (> edges 0)
                                 (/ inter (float edges))
                                 0.0))))

(defun %signalograd-swarm-observation ()
  (let ((scores (ignore-errors (if (fboundp '%load-swarm-scores)
                                   (%load-swarm-scores)
                                   '()))))
    (if (null scores)
        (list :success 0.5 :latency 0.0 :cost 0.0)
        (let ((success 0.0)
              (latency 0.0)
              (cost 0.0)
              (count 0)
              (latency-reference (max 1.0 (%signalograd-policy-number "swarm/latency-reference-ms" 8000.0)))
              (cost-scale (%signalograd-policy-number "swarm/cost-scale" 20.0)))
          (dolist (entry scores)
            (incf count)
            (incf success (or (getf entry :success-rate) 0.5))
            (incf latency (or (getf entry :latency-ms) 0.0))
            (incf cost (or (getf entry :cost-avg) 0.0)))
          (list :success (/ success (max 1 count))
                :latency (%signalograd-clamp (/ latency (* (max 1 count) latency-reference)) 0.0 1.0)
                :cost (%signalograd-clamp (* (/ cost (max 1 count)) cost-scale) 0.0 1.0))))))

(defun %signalograd-reward (ctx runtime)
  (let* ((plan (getf ctx :plan))
         (vit (and plan (getf plan :vitruvian)))
         (signal (or (and vit (getf vit :signal)) 0.0))
         (noise (or (and vit (getf vit :noise)) 1.0))
         (chaos (or (getf (getf ctx :logistic) :chaos-risk) 1.0))
         (error-max (max 1.0 (%signalograd-policy-number "reward/max-errors" 10.0)))
         (queue-max (max 1.0 (%signalograd-policy-number "reward/max-queue-depth" 10.0)))
         (errors (%signalograd-clamp (/ *consecutive-tick-errors* error-max) 0.0 1.0))
         (queue (%signalograd-clamp (/ (if runtime
                                           (length (runtime-state-prompt-queue runtime))
                                           0)
                                       queue-max)
                                    0.0 1.0)))
    (%signalograd-clamp (- signal
                           (* (%signalograd-policy-number "reward/noise-weight" 0.6) noise)
                           (* (%signalograd-policy-number "reward/chaos-weight" 0.4) chaos)
                           (* (%signalograd-policy-number "reward/error-weight" 0.3) errors)
                           (* (%signalograd-policy-number "reward/queue-weight" 0.2) queue))
                        0.0 1.0)))

(defun %signalograd-stability (ctx)
  (let* ((chaos (or (getf (getf ctx :logistic) :chaos-risk) 1.0))
         (ratio (or (getf (getf ctx :projection) :ratio) 0.0))
         (bounded (or (getf (getf ctx :lorenz) :bounded-score) 0.0)))
    (%signalograd-clamp (+ (* (%signalograd-policy-number "stability/chaos-weight" 0.45)
                              (- 1.0 chaos))
                           (* (%signalograd-policy-number "stability/ratio-weight" 0.30)
                              ratio)
                           (* (%signalograd-policy-number "stability/bounded-weight" 0.25)
                              bounded))
                        0.0 1.0)))

(defun %signalograd-novelty (ctx)
  (let* ((map (getf ctx :map))
         (stats (%signalograd-graph-stats map)))
    (%signalograd-clamp (+ (* (%signalograd-policy-number "novelty/interdisciplinary-weight" 0.6)
                              (getf stats :interdisciplinary))
                           (* (%signalograd-policy-number "novelty/density-weight" 0.4)
                              (min 1.0
                                   (* (%signalograd-policy-number "novelty/density-scale" 8.0)
                                      (getf stats :density)))))
                        0.0 1.0)))

(defun %signalograd-security-posture-string (ctx)
  (let ((security (getf ctx :security)))
    (string-downcase (symbol-name (or (getf security :posture) :nominal)))))

(defun %signalograd-actor-metrics (&optional (runtime *runtime*))
  (let ((running 0)
        (stall-sum 0)
        (pending (if runtime (length (runtime-state-actor-pending runtime)) 0)))
    (when runtime
      (maphash (lambda (_id record)
                 (declare (ignore _id))
                 (when (member (actor-record-state record) '(:spawning :running))
                   (incf running)
                   (incf stall-sum (max 0 (or (actor-record-stall-ticks record) 0)))))
               (runtime-state-actor-registry runtime)))
    (list :load (float (max running pending))
          :stalls (float stall-sum)
          :pending pending)))

(defun %signalograd-error-pressure ()
  (let* ((consecutive-scale (max 1.0 (%signalograd-policy-number "telemetry/error-consecutive-scale" 6.0)))
         (total-scale (max 1.0 (%signalograd-policy-number "telemetry/error-total-scale" 24.0)))
         (consecutive (%signalograd-clamp (/ *consecutive-tick-errors* consecutive-scale) 0.0 1.0))
         (total (%signalograd-clamp (/ *tick-error-count* total-scale) 0.0 1.0)))
    (%signalograd-clamp (+ (* 0.7 consecutive) (* 0.3 total)) 0.0 1.0)))

(defun %signalograd-presentation-observation (&optional (runtime *runtime*))
  (let* ((telemetry (and runtime (runtime-state-last-response-telemetry runtime)))
         (verbosity-scale (max 1.0
                               (%signalograd-policy-number
                                "telemetry/presentation-verbosity-reference-words" 120.0))))
    (list :cleanliness (or (and telemetry (getf telemetry :cleanliness)) 1.0)
          :verbosity (%signalograd-clamp
                      (/ (or (and telemetry (getf telemetry :verbosity)) 0)
                         (float verbosity-scale))
                      0.0 1.0)
          :markdown-density (or (and telemetry (getf telemetry :markdown-density)) 0.0)
          :symbolic-density (or (and telemetry (getf telemetry :symbolic-density)) 0.0)
          :self-reference (or (and telemetry (getf telemetry :self-reference)) 0.0)
          :decor-density (or (and telemetry (getf telemetry :decor-density)) 0.0)
          :user-affinity (%presentation-user-affinity runtime))))

(defun %signalograd-observation-sexp (ctx &optional (runtime *runtime*))
  (let* ((global (getf ctx :global))
         (local (getf ctx :local))
         (plan (getf ctx :plan))
         (vit (and plan (getf plan :vitruvian)))
         (logistic (getf ctx :logistic))
         (lorenz (getf ctx :lorenz))
         (projection (getf ctx :projection))
         (security (getf ctx :security))
         (map (getf ctx :map))
         (graph (%signalograd-graph-stats map))
         (swarm (%signalograd-swarm-observation))
         (telemetry (%signalograd-actor-metrics runtime))
         (presentation (%signalograd-presentation-observation runtime))
         (queue-depth (if runtime (length (runtime-state-prompt-queue runtime)) 0))
         (prior-confidence (or (getf (%signalograd-projection runtime) :confidence) 0.0))
         (reward (%signalograd-reward ctx runtime))
         (stability (%signalograd-stability ctx))
         (novelty (%signalograd-novelty ctx)))
    (%signalograd-sexp
     :signalograd-observe
     :cycle (or (getf ctx :cycle) 0)
     :global-score (or (getf global :global-score) 0.0)
     :local-score (or (getf local :local-score) 0.0)
     :signal (or (and vit (getf vit :signal)) 0.0)
     :noise (or (and vit (getf vit :noise)) 1.0)
     :chaos-risk (or (getf logistic :chaos-risk) 1.0)
     :rewrite-aggression (or (getf logistic :rewrite-aggression) 0.0)
     :lorenz-bounded (or (getf lorenz :bounded-score) 0.0)
     :lambdoma-ratio (or (getf projection :ratio) 0.0)
     :rewrite-ready (and plan (getf plan :ready))
     :security-posture (%signalograd-security-posture-string ctx)
     :security-events (or (and security (getf security :events)) 0.0)
     :route-success (getf swarm :success)
     :route-latency (getf swarm :latency)
     :cost-pressure (getf swarm :cost)
     :memory-pressure (%signalograd-clamp (- 1.0 reward) 0.0 1.0)
     :graph-density (getf graph :density)
     :graph-interdisciplinary (getf graph :interdisciplinary)
     :reward reward
     :stability stability
     :novelty novelty
     :actor-load (getf telemetry :load)
     :actor-stalls (getf telemetry :stalls)
     :queue-depth queue-depth
     :error-pressure (%signalograd-error-pressure)
     :supervision (%supervision-rate)
     :prior-confidence prior-confidence
     :presentation-cleanliness (getf presentation :cleanliness)
     :presentation-verbosity (getf presentation :verbosity)
     :presentation-markdown-density (getf presentation :markdown-density)
     :presentation-symbolic-density (getf presentation :symbolic-density)
     :presentation-self-reference (getf presentation :self-reference)
     :presentation-decor-density (getf presentation :decor-density)
     :presentation-user-affinity (getf presentation :user-affinity)
     :route-tier (symbol-name (or *routing-tier* :auto)))))

(defun %signalograd-feedback-plist (ctx &optional (runtime *runtime*))
  (let* ((projection (%signalograd-projection runtime))
         (confidence (or (getf projection :confidence) 0.0))
         (telemetry (and runtime (runtime-state-last-response-telemetry runtime)))
         (cleanliness (or (and telemetry (getf telemetry :cleanliness)) 1.0))
         (user-affinity (%presentation-user-affinity runtime)))
    (when (> confidence 0.0)
      (let* ((reward (%signalograd-reward ctx runtime))
             (stability (%signalograd-stability ctx))
             (novelty (%signalograd-novelty ctx))
             (recall-strength (or (getf projection :recall-strength) 0.0))
             (accepted (and (>= reward (%signalograd-policy-number "feedback/reward-accept-min" 0.58))
                            (>= stability (%signalograd-policy-number "feedback/stability-accept-min" 0.55))
                            (>= user-affinity (%signalograd-policy-number "feedback/user-affinity-accept-min" 0.35))
                            (>= cleanliness (%signalograd-policy-number "feedback/cleanliness-accept-min" 0.55))))
             (recall-hits (if (>= recall-strength
                                  (%signalograd-policy-number "feedback/recall-strength-hit-min" 0.12))
                              1
                              0)))
        (list :cycle (or (getf ctx :cycle) 0)
              :reward reward
              :stability stability
              :novelty novelty
              :accepted accepted
              :recall-hits recall-hits
              :user-affinity user-affinity
              :cleanliness cleanliness
              :applied-confidence confidence)))))

(defun %signalograd-feedback-sexp (ctx &optional (runtime *runtime*))
  (let ((plist (%signalograd-feedback-plist ctx runtime)))
    (when plist
      (apply #'%signalograd-sexp :signalograd-feedback plist))))

(defun signalograd-dispatch-reflection (ctx &key (runtime *runtime*))
  "Send one compact reflection observation to the Rust kernel.
The kernel emits its proposal back through the unified actor mailbox."
  (when (and runtime (fboundp 'signalograd-port-ready-p) (signalograd-port-ready-p))
    (let* ((feedback-plist (%signalograd-feedback-plist ctx runtime))
           (feedback-sexp (when feedback-plist
                            (apply #'%signalograd-sexp :signalograd-feedback feedback-plist))))
      (when feedback-sexp
        (when (%trace-level-p :standard)
          (trace-event "signalograd-feedback" :chain
                       :metadata (list :reward (getf feedback-plist :reward)
                                       :user-affinity (getf feedback-plist :user-affinity)
                                       :recall-hits (getf feedback-plist :recall-hits)
                                       :accepted (getf feedback-plist :accepted))))
        (ignore-errors
          (signalograd-feedback feedback-sexp)
          (%signalograd-record-event
           "feedback"
           :cycle (getf feedback-plist :cycle)
           :confidence (getf feedback-plist :applied-confidence)
           :stability (getf feedback-plist :stability)
           :novelty (getf feedback-plist :novelty)
           :reward (getf feedback-plist :reward)
           :accepted (getf feedback-plist :accepted)
           :recall-hits (getf feedback-plist :recall-hits)
           :detail (list :user-affinity (getf feedback-plist :user-affinity)
                         :cleanliness (getf feedback-plist :cleanliness)
                          :feedback feedback-plist)))))
    (let ((observation-sexp (%signalograd-observation-sexp ctx runtime)))
      (ignore-errors
        (signalograd-observe observation-sexp)
        (%signalograd-record-event
         "observe"
         :cycle (or (getf ctx :cycle) 0)
         :reward (%signalograd-reward ctx runtime)
         :stability (%signalograd-stability ctx)
         :novelty (%signalograd-novelty ctx)
         :detail (list :observation observation-sexp))))))

(defun signalograd-checkpoint-latest (&key (runtime *runtime*))
  (when (and (fboundp 'signalograd-port-ready-p) (signalograd-port-ready-p))
    (let ((path (%signalograd-latest-checkpoint-path)))
      (when path
        (signalograd-checkpoint (namestring path))
        (let* ((status (ignore-errors (signalograd-status)))
               (digest (and (listp status) (getf status :checkpoint-digest))))
          (when runtime
            (runtime-log runtime :signalograd-checkpoint
                         (list :path (namestring path)
                               :cycle (and (listp status) (getf status :cycle))
                               :digest digest)))
          (%signalograd-record-event
           "checkpoint"
           :cycle (and (listp status) (getf status :cycle))
           :confidence (and (listp status) (getf status :confidence))
           :stability (and (listp status) (getf status :stability))
           :novelty (and (listp status) (getf status :novelty))
           :checkpoint-path (namestring path)
           :checkpoint-digest digest
           :detail (list :target :latest)))
        path))))

(defun signalograd-restore-for-current-evolution (&key (runtime *runtime*))
  (when (and (fboundp 'signalograd-port-ready-p) (signalograd-port-ready-p))
    (let* ((version (and (fboundp 'evolution-current-version)
                         (evolution-current-version)))
           (version-path (%signalograd-version-checkpoint-path version))
           (latest-path (%signalograd-latest-checkpoint-path))
           (selected (cond
                       ((and version-path (probe-file version-path)) version-path)
                       ((and latest-path (probe-file latest-path)) latest-path)
                       (t nil))))
      (when selected
        (signalograd-restore (namestring selected))
        (let* ((status (ignore-errors (signalograd-status)))
               (digest (and (listp status) (getf status :checkpoint-digest))))
          (when runtime
            (runtime-log runtime :signalograd-restore
                         (list :path (namestring selected)
                               :version version
                               :cycle (and (listp status) (getf status :cycle))
                               :digest digest)))
          (%signalograd-record-event
           "restore"
           :cycle (and (listp status) (getf status :cycle))
           :confidence (and (listp status) (getf status :confidence))
           :stability (and (listp status) (getf status :stability))
           :novelty (and (listp status) (getf status :novelty))
           :checkpoint-path (namestring selected)
           :checkpoint-digest digest
           :detail (list :source (if (and version-path (equal selected version-path))
                                     :version
                                     :latest)
                         :version version)))
        selected))))

(defun signalograd-adjust-vitruvian (vitruvian &optional (runtime *runtime*))
  (let* ((harmony (%signalograd-section :harmony runtime))
         (signal-bias (if harmony (or (getf harmony :signal-bias) 0.0) 0.0))
         (noise-bias (if harmony (or (getf harmony :noise-bias) 0.0) 0.0))
         (signal (%signalograd-clamp (+ (or (getf vitruvian :signal) 0.0) signal-bias) 0.0 1.0))
         (noise (%signalograd-clamp (+ (or (getf vitruvian :noise) 0.0) noise-bias) 0.0 1.0)))
    (append vitruvian (list :signal signal :noise noise))))

(defun signalograd-effective-harmony-number (path default &optional (runtime *runtime*))
  (let* ((base (harmony-policy-number path default))
         (harmony (%signalograd-section :harmony runtime))
         (security (%signalograd-section :security-shell runtime)))
    (cond
      ((string-equal path "rewrite-plan/signal-min")
       (%signalograd-clamp (+ base (or (and harmony (getf harmony :rewrite-signal-delta)) 0.0))
                           (%signalograd-policy-number "limits/rewrite-signal-min" 0.20)
                           (%signalograd-policy-number "limits/rewrite-signal-max" 0.95)))
      ((string-equal path "rewrite-plan/chaos-max")
       (%signalograd-clamp (+ base (or (and harmony (getf harmony :rewrite-chaos-delta)) 0.0))
                           (%signalograd-policy-number "limits/rewrite-chaos-min" 0.05)
                           (%signalograd-policy-number "limits/rewrite-chaos-max" 0.95)))
      ((string-equal path "memory/crystal-min-score")
       (%signalograd-clamp (+ base (or (signalograd-memory-crystal-threshold-delta runtime) 0.0))
                           (%signalograd-policy-number "limits/memory-crystal-min" 0.10)
                           (%signalograd-policy-number "limits/memory-crystal-max" 0.98)))
      ((string-equal path "security/dissonance-weight")
       (%signalograd-clamp (+ base (or (and security (getf security :dissonance-weight-delta)) 0.0))
                           (%signalograd-policy-number "limits/security-dissonance-min" 0.05)
                           (%signalograd-policy-number "limits/security-dissonance-max" 0.95)))
      ((string-equal path "security/anomaly-threshold-stddev")
       (%signalograd-clamp (+ base (or (and security (getf security :anomaly-threshold-delta)) 0.0))
                           (%signalograd-policy-number "limits/security-anomaly-min" 0.50)
                           (%signalograd-policy-number "limits/security-anomaly-max" 4.00)))
      (t base))))

(defun signalograd-memory-recall-limit (&optional (runtime *runtime*))
  (let ((delta (%signalograd-section-value :memory :recall-limit-delta 0 runtime)))
    (truncate (%signalograd-clamp (+ (harmony-policy-number "memory/recall-limit" 5) delta)
                                  (%signalograd-policy-number "limits/memory-recall-min" 2.0)
                                  (%signalograd-policy-number "limits/memory-recall-max" 12.0)))))

(defun signalograd-memory-bootstrap-skill-limit (&optional (runtime *runtime*))
  (let ((delta (%signalograd-section-value :memory :recall-limit-delta 0 runtime)))
    (truncate (%signalograd-clamp (+ (harmony-policy-number "memory/bootstrap-skill-limit" 3)
                                     (if (> delta 0) 1 0))
                                  (%signalograd-policy-number "limits/memory-bootstrap-min" 1.0)
                                  (%signalograd-policy-number "limits/memory-bootstrap-max" 8.0)))))

(defun signalograd-memory-crystal-threshold-delta (&optional (runtime *runtime*))
  (%signalograd-section-value :memory :crystal-threshold-delta 0.0 runtime))

(defun signalograd-routing-weight (metric base &optional (runtime *runtime*))
  (let* ((routing (%signalograd-section :routing runtime))
         (delta (cond
                  ((eq metric :price) (or (and routing (getf routing :price-weight-delta)) 0.0))
                  ((eq metric :speed) (or (and routing (getf routing :speed-weight-delta)) 0.0))
                  ((eq metric :success) (or (and routing (getf routing :success-weight-delta)) 0.0))
                  ((eq metric :reasoning) (or (and routing (getf routing :reasoning-weight-delta)) 0.0))
                  (t 0.0))))
    (%signalograd-clamp (+ base delta)
                        (%signalograd-policy-number "limits/routing-weight-min" 0.05)
                        (%signalograd-policy-number "limits/routing-weight-max" 0.70))))

(defun signalograd-routing-vitruvian-min (&optional (runtime *runtime*))
  (let ((base (or (getf *model-evolution-policy* :vitruvian-signal-min) 0.62)))
    (%signalograd-clamp (+ base (%signalograd-section-value :routing :vitruvian-min-delta 0.0 runtime))
                        (%signalograd-policy-number "limits/routing-vitruvian-min" 0.30)
                        (%signalograd-policy-number "limits/routing-vitruvian-max" 0.95))))

(defun signalograd-adjust-aggression (value &optional (runtime *runtime*))
  (%signalograd-clamp (+ value (%signalograd-section-value :harmony :aggression-bias 0.0 runtime))
                      (%signalograd-policy-number "limits/aggression-min" 0.01)
                      (%signalograd-policy-number "limits/aggression-max" 0.99)))

(defun signalograd-presentation-value (metric default &optional (runtime *runtime*))
  (let ((presentation (%signalograd-section :presentation runtime)))
    (if (and presentation (getf presentation metric))
        (getf presentation metric)
        default)))
