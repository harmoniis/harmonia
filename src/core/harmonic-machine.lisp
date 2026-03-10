;;; harmonic-machine.lisp — Harmonic evolution state machine.

(in-package :harmonia)

(defparameter *harmonic-phases*
  '(:observe :evaluate-global :evaluate-local :logistic-balance :lambdoma-project :attractor-sync :rewrite-plan :security-audit :stabilize))

;;; --- Wave 5: Security posture tracking ---
(defparameter *security-posture* :nominal
  "Current security posture: :nominal, :elevated, or :alert.")
(defparameter *security-event-count* 0
  "Count of security events since last audit.")
(defparameter *security-injection-counts* (make-hash-table :test 'equal)
  "Per-frontend injection attempt counts for behavioral baseline.")

(defun security-note-event (&key frontend (injection-count 0))
  "Record a security-relevant event for the next :security-audit phase."
  (incf *security-event-count*)
  (when (and frontend (> injection-count 0))
    (incf (gethash frontend *security-injection-counts* 0) injection-count))
  t)

(defun %clamp (x lo hi)
  (max lo (min hi x)))

(defun %safe-div (a b)
  (if (zerop b) 0.0 (/ a (float b))))

(defun %occam-pass-rate ()
  (let* ((skills (memory-recent :class :skill :limit 256))
         (total (length skills))
         (pass 0))
    (dolist (entry skills)
      (let* ((content (memory-entry-content entry))
             (harmony (and (listp content) (getf content :harmony))))
        (when (and harmony (getf harmony :occam-pass))
          (incf pass))))
    (%safe-div pass (max 1 total))))

(defun %node-index (nodes)
  (let ((idx (make-hash-table :test 'equal)))
    (dolist (n nodes)
      (setf (gethash (getf n :concept) idx) n))
    idx))

(defun %edge-simplicity (edge idx)
  (let* ((na (gethash (getf edge :a) idx))
         (nb (gethash (getf edge :b) idx))
         (ca (if na (max 1 (getf na :count)) 1))
         (cb (if nb (max 1 (getf nb :count)) 1))
         (mn (min ca cb))
         (mx (max ca cb)))
    (%safe-div mn mx)))

(defun %global-harmony (map)
  (let* ((nodes (getf map :concept-nodes))
         (edges (getf map :concept-edges))
         (edge-n (length edges))
         (idx (%node-index nodes))
         (inter 0)
         (simp-sum 0.0))
    (dolist (e edges)
      (when (getf e :interdisciplinary) (incf inter))
      (incf simp-sum (%edge-simplicity e idx)))
    (let* ((simp (%safe-div simp-sum (max 1 edge-n)))
           (inter-ratio (%safe-div inter (max 1 edge-n)))
           (occam (%occam-pass-rate))
           (global (+ (* 0.45 simp) (* 0.30 occam) (* 0.25 inter-ratio))))
      (list :simplicity simp
            :interdisciplinary-ratio inter-ratio
            :occam-pass-rate occam
            :global-score (%clamp global 0.0 1.0)))))

(defun %denoise-map (map)
  (let* ((nodes (remove-if (lambda (n) (< (getf n :count) 2))
                           (getf map :concept-nodes)))
         (edges (remove-if (lambda (e) (< (getf e :weight) 2))
                           (getf map :concept-edges))))
    (if (or (null nodes) (null edges))
        map
        (append (list :concept-nodes nodes :concept-edges edges)
                (loop for (k v) on map by #'cddr
                      unless (member k '(:concept-nodes :concept-edges))
                      append (list k v))))))

(defun %best-focus-concept (map)
  (let ((best nil) (best-count -1))
    (dolist (n (getf map :concept-nodes))
      (when (> (getf n :count) best-count)
        (setf best n
              best-count (getf n :count))))
    (if best (getf best :concept) "")))

(defun %local-fractal-harmony (map focus)
  (let* ((nodes (getf map :concept-nodes))
         (edges (getf map :concept-edges))
         (idx (%node-index nodes))
         (local-edges
          (remove-if-not (lambda (e)
                           (or (string= focus (getf e :a))
                               (string= focus (getf e :b))))
                         edges))
         (n (length local-edges))
         (simp-sum 0.0)
         (cross 0))
    (dolist (e local-edges)
      (incf simp-sum (%edge-simplicity e idx))
      (when (getf e :interdisciplinary) (incf cross)))
    (let ((simp (%safe-div simp-sum (max 1 n)))
          (cross-ratio (%safe-div cross (max 1 n))))
      (list :focus focus
            :edge-count n
            :simplicity simp
            :interdisciplinary-ratio cross-ratio
            :local-score (%clamp (+ (* 0.6 simp) (* 0.4 cross-ratio)) 0.0 1.0)))))

(defun %logistic-next (x r)
  (* r x (- 1.0 x)))

(defun %step-logistic (runtime)
  (let* ((x (runtime-state-harmonic-x runtime))
         (r (runtime-state-harmonic-r runtime))
         (x2 (%clamp (%logistic-next x r) 0.0001 0.9999))
         (edge (harmony-policy-number "logistic/edge" 3.56995))
         (distance (abs (- r edge)))
         (window (harmony-policy-number "logistic/distance-window" 0.4))
         (chaos-risk (%clamp (%safe-div (- window distance) window) 0.0 1.0))
         (ag-base (harmony-policy-number "logistic/aggression-base" 0.35))
         (ag-scale (harmony-policy-number "logistic/aggression-scale" 0.65))
         (ag-min (harmony-policy-number "logistic/aggression-min" 0.05))
         (ag-max (harmony-policy-number "logistic/aggression-max" 0.95))
         (aggression (%clamp (* (- 1.0 chaos-risk) (+ ag-base (* ag-scale x2))) ag-min ag-max)))
    (setf (runtime-state-harmonic-x runtime) x2)
    (list :x x2 :r r :chaos-risk chaos-risk :rewrite-aggression aggression)))

(defun %lambdoma-convergence (global local)
  (let* ((g (getf global :global-score))
         (l (getf local :local-score))
         (mn (min g l))
         (mx (max g l))
         (ratio (%safe-div mn (max 0.0001 mx))))
    (list :global g :local l :ratio ratio
          :convergent-p (>= ratio (harmony-policy-number "lambdoma/convergence-min" 0.72)))))

(defun %complexity-balance (map)
  (let* ((nodes (max 1 (length (getf map :concept-nodes))))
         (edges (length (getf map :concept-edges)))
         (density (%safe-div edges (* nodes nodes)))
         ;; Simple things simple (low unnecessary density) + complex things possible (not trivial zero density).
         (simple-score (%clamp (- 1.0 (* (harmony-policy-number "complexity/density-simple-mult" 3.0) density)) 0.0 1.0))
         (possible-score (%clamp (* (harmony-policy-number "complexity/density-possible-mult" 8.0) density) 0.0 1.0)))
    (list :density density
          :simple-things-simple simple-score
          :complex-things-possible possible-score
          :balance (%clamp (* 0.5 (+ simple-score possible-score)) 0.0 1.0))))

(defun %tool-memory-coherence (map)
  (let* ((edges (getf map :concept-edges))
         (total 0)
         (coherent 0))
    (dolist (e edges)
      (let ((a (getf e :a))
            (b (getf e :b)))
        (when (or (search "tool" a :test #'char-equal)
                  (search "tool" b :test #'char-equal)
                  (member a '("openrouter" "git-ops" "memory") :test #'string=)
                  (member b '("openrouter" "git-ops" "memory") :test #'string=))
          (incf total)
          (when (or (search "memory" a :test #'char-equal)
                    (search "memory" b :test #'char-equal)
                    (search "skill" a :test #'char-equal)
                    (search "skill" b :test #'char-equal))
            (incf coherent)))))
    (%safe-div coherent (max 1 total))))

(defun %step-lorenz (runtime)
  (let* ((sigma (harmony-policy-number "lorenz/sigma" 10.0))
         (rho (harmony-policy-number "lorenz/rho" 28.0))
         (beta (harmony-policy-number "lorenz/beta" (/ 8.0 3.0)))
         (dt (harmony-policy-number "lorenz/dt" 0.01))
         (x (runtime-state-lorenz-x runtime))
         (y (runtime-state-lorenz-y runtime))
         (z (runtime-state-lorenz-z runtime))
         (dx (* sigma (- y x)))
         (dy (- (* x (- rho z)) y))
         (dz (- (* x y) (* beta z)))
         (x2 (+ x (* dt dx)))
         (y2 (+ y (* dt dy)))
         (z2 (+ z (* dt dz))))
    (setf (runtime-state-lorenz-x runtime) x2)
    (setf (runtime-state-lorenz-y runtime) y2)
    (setf (runtime-state-lorenz-z runtime) z2)
    ;; Basin metric: prefer bounded-but-dynamic region.
    (let* ((radius (sqrt (+ (* x2 x2) (* y2 y2) (* z2 z2))))
           (target (harmony-policy-number "lorenz/target-radius" 25.0))
           (window (harmony-policy-number "lorenz/radius-window" 25.0))
           (bounded (%clamp (- 1.0 (%safe-div (abs (- radius target)) window)) 0.0 1.0)))
      (list :x x2 :y y2 :z z2 :radius radius :bounded-score bounded))))

(defun %vitruvian-scores (global local projection logistic lorenz map)
  (let* ((strength (%clamp (+ (* (harmony-policy-number "vitruvian/strength-chaos-weight" 0.6) (- 1.0 (getf logistic :chaos-risk)))
                              (* (harmony-policy-number "vitruvian/strength-bounded-weight" 0.4) (getf lorenz :bounded-score)))
                           0.0 1.0))
         (utility (%clamp (+ (* (harmony-policy-number "vitruvian/utility-global-weight" 0.45) (getf global :global-score))
                             (* (harmony-policy-number "vitruvian/utility-coherence-weight" 0.30) (%tool-memory-coherence map))
                             (* (harmony-policy-number "vitruvian/utility-balance-weight" 0.25) (getf (%complexity-balance map) :balance)))
                          0.0 1.0))
         (beauty (%clamp (+ (* (harmony-policy-number "vitruvian/beauty-ratio-weight" 0.50) (getf projection :ratio))
                            (* (harmony-policy-number "vitruvian/beauty-inter-weight" 0.25) (getf local :interdisciplinary-ratio))
                            (* (harmony-policy-number "vitruvian/beauty-simplicity-weight" 0.25) (getf global :simplicity)))
                         0.0 1.0))
         (signal (%clamp (+ (* (harmony-policy-number "vitruvian/signal-strength-weight" 0.34) strength)
                            (* (harmony-policy-number "vitruvian/signal-utility-weight" 0.33) utility)
                            (* (harmony-policy-number "vitruvian/signal-beauty-weight" 0.33) beauty))
                         0.0 1.0))
         (noise (- 1.0 signal)))
    (list :strength strength :utility utility :beauty beauty :signal signal :noise noise)))

(defun %next-phase (phase)
  (case phase
    (:observe :evaluate-global)
    (:evaluate-global :evaluate-local)
    (:evaluate-local :logistic-balance)
    (:logistic-balance :lambdoma-project)
    (:lambdoma-project :attractor-sync)
    (:attractor-sync :rewrite-plan)
    (:rewrite-plan :security-audit)
    (:security-audit :stabilize)
    (t :observe)))

(defun harmonic-state-step (&key (runtime *runtime*))
  "One phase transition per tick. Global + local harmonic checks stay coupled."
  (let* ((phase (runtime-state-harmonic-phase runtime))
         (ctx (runtime-state-harmonic-context runtime)))
    (case phase
      (:observe
       (setf (runtime-state-harmonic-context runtime)
             (list :map (%denoise-map (memory-map-sexp :entry-limit 120 :edge-limit 160))
                   :cycle (runtime-state-cycle runtime)))
       (setf (runtime-state-harmonic-phase runtime) (%next-phase phase)))
      (:evaluate-global
       (let* ((map (getf ctx :map))
              (global (%global-harmony map)))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :global global) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:evaluate-local
       (let* ((map (getf ctx :map))
              (focus (%best-focus-concept map))
              (local (%local-fractal-harmony map focus)))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :local local) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:logistic-balance
       (let ((logistic (%step-logistic runtime)))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :logistic logistic) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:lambdoma-project
       (let* ((global (getf ctx :global))
              (local (getf ctx :local))
              (projection (%lambdoma-convergence global local)))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :projection projection) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:attractor-sync
       (let ((lorenz (%step-lorenz runtime)))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :lorenz lorenz) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:rewrite-plan
       (let* ((projection (getf ctx :projection))
              (logistic (getf ctx :logistic))
              (global (getf ctx :global))
              (local (getf ctx :local))
              (map (getf ctx :map))
              (lorenz (getf ctx :lorenz))
              (vitruvian (%vitruvian-scores global local projection logistic lorenz map))
              (ok (and (getf projection :convergent-p)
                       (< (getf logistic :chaos-risk)
                          (harmony-policy-number "rewrite-plan/chaos-max" 0.55))
                       (>= (getf vitruvian :signal)
                           (harmony-policy-number "rewrite-plan/signal-min" 0.62))))
              (plan (list :state-machine :harmonic
                          :ready ok
                          :focus (getf (getf ctx :local) :focus)
                          :aggression (getf logistic :rewrite-aggression)
                          :lambdoma-ratio (getf projection :ratio)
                          :vitruvian vitruvian
                          :dna-laws (getf *dna* :laws))))
         (when ok
           (incf (runtime-state-rewrite-count runtime)))
         (runtime-log runtime :harmonic-rewrite-plan plan)
         (setf (runtime-state-harmonic-context runtime)
               (append (list :plan plan) ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:security-audit
       ;; Wave 5.2: Security audit phase — silent, runs as part of harmonic cycle
       (let* ((event-count *security-event-count*)
              (injection-total 0)
              (worst-frontend nil)
              (worst-count 0))
         ;; Scan per-frontend injection counts
         (maphash (lambda (frontend count)
                    (incf injection-total count)
                    (when (> count worst-count)
                      (setf worst-frontend frontend)
                      (setf worst-count count)))
                  *security-injection-counts*)
         ;; Update posture
         (let ((new-posture (cond
                              ((> injection-total 20) :alert)
                              ((> injection-total 5) :elevated)
                              (t :nominal))))
           (setf *security-posture* new-posture)
           ;; Reset counters for next audit cycle
           (setf *security-event-count* 0)
           (clrhash *security-injection-counts*))
         (runtime-log runtime :security-audit
                      (list :events event-count
                            :injection-total injection-total
                            :worst-frontend worst-frontend
                            :posture *security-posture*))
         (setf (runtime-state-harmonic-context runtime)
               (append (list :security (list :events event-count
                                             :posture *security-posture*))
                       ctx))
         (setf (runtime-state-harmonic-phase runtime) (%next-phase phase))))
      (:stabilize
       (runtime-log runtime :harmonic-stabilized
                    (list :phase :stabilize
                          :rewrite-count (runtime-state-rewrite-count runtime)))
       ;; Chronicle: record full harmonic state + concept graph snapshot
       (ignore-errors (chronicle-record-harmonic ctx))
       (ignore-errors (chronicle-record-graph-snapshot))
       (setf (runtime-state-harmonic-phase runtime) :observe))
      (t
       (setf (runtime-state-harmonic-phase runtime) :observe)))
    runtime))
