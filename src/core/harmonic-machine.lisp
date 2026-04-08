;;; harmonic-machine.lisp — Harmonic evolution state machine.
;;; All weights flow through harmony-policy (configurable) and signalograd (adaptive).
;;; No hardcoded numeric weights — every blend uses policy-weighted-sum.

(in-package :harmonia)

(defun %policy-weighted-sum (components)
  "Pure functional weighted blend. COMPONENTS is a list of (policy-key default-weight value).
   Weights come from harmony-policy, making them configurable and signalograd-adaptive.
   Returns the clamped [0,1] weighted sum."
  (%clamp (reduce #'+ components :key
                  (lambda (c) (* (harmony-policy-number (first c) (second c)) (third c)))
                  :initial-value 0.0)
          0.0 1.0))

;;; ─── Tonal Function Theory (Riemann/Türk Funktionstheorie) ────────
;;;
;;; Each phase belongs to a tonal function:
;;;   T (Tonic)       — stability, home, sensing
;;;   S (Subdominant) — preparation, risk computation
;;;   D (Dominant)    — tension, decisions, must resolve
;;;   R (Resolution)  — cadence, persist, return to T
;;;
;;; Progression constraints (from harmonic theory):
;;;   T → S (standard), S → D (standard), D → R (authentic cadence)
;;;   S → R (plagal cadence — skip dominant when low risk)
;;;   D → T (deceptive cadence — security escalation re-evaluates)
;;;
;;; Pythagorean ratios govern weight relationships:
;;;   Vitruvian triad: ~1:1:1 (unison), Global: ~3:2:5/3 (fifth:fourth)
;;;   Strength: 3:2 (perfect fifth), Beauty: 2:1:1 (octave)

(defparameter *tonal-functions*
  '((:observe          . :tonic)
    (:evaluate-global  . :tonic)
    (:evaluate-local   . :tonic)
    (:logistic-balance . :subdominant)
    (:lambdoma-project . :subdominant)
    (:attractor-sync   . :dominant)
    (:rewrite-plan     . :dominant)
    (:security-audit   . :dominant)
    (:stabilize        . :resolution))
  "Tonal function classification for each harmonic phase.
   Enforces Funktionstheorie: D must resolve, S prepares D, T is home.")

(defparameter *harmonic-phases*
  '((:observe          . %phase-observe)
    (:evaluate-global  . %phase-evaluate-global)
    (:evaluate-local   . %phase-evaluate-local)
    (:logistic-balance . %phase-logistic-balance)
    (:lambdoma-project . %phase-lambdoma-project)
    (:attractor-sync   . %phase-attractor-sync)
    (:rewrite-plan     . %phase-rewrite-plan)
    (:security-audit   . %phase-security-audit)
    (:stabilize        . %phase-stabilize))
  "Dispatch table: phase keyword → pure handler function.")

(defun %tonal-function (phase)
  "Return the tonal function (:tonic :subdominant :dominant :resolution) for a phase."
  (or (cdr (assoc phase *tonal-functions*)) :tonic))

(defun %cadence-type (from-function to-function)
  "Classify the cadence type of a phase transition.
   Returns :authentic (D→R), :plagal (S→R), :deceptive (D→T), :standard, or :retrogression."
  (cond
    ((and (eq from-function :dominant) (eq to-function :resolution)) :authentic)
    ((and (eq from-function :subdominant) (eq to-function :resolution)) :plagal)
    ((and (eq from-function :dominant) (eq to-function :tonic)) :deceptive)
    ((and (eq from-function :resolution) (eq to-function :tonic)) :standard)
    ((and (eq from-function :tonic) (eq to-function :subdominant)) :standard)
    ((and (eq from-function :subdominant) (eq to-function :dominant)) :standard)
    ((and (eq from-function :tonic) (eq to-function :dominant)) :standard)
    ((and (eq from-function :dominant) (eq to-function :subdominant)) :retrogression)
    (t :standard)))

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
         (total (length skills)))
    (%safe-div (count-if (lambda (entry)
                           (let* ((content (memory-entry-content entry))
                                  (harmony (and (listp content) (getf content :harmony))))
                             (and harmony (getf harmony :occam-pass))))
                         skills)
               (max 1 total))))

(defun %node-index (nodes)
  (reduce (lambda (idx n)
            (setf (gethash (getf n :concept) idx) n)
            idx)
          nodes
          :initial-value (make-hash-table :test 'equal)))

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
         (inter (count-if (lambda (e) (getf e :interdisciplinary)) edges))
         (simp-sum (reduce (lambda (acc e) (+ acc (%edge-simplicity e idx)))
                           edges :initial-value 0.0))
         (simp (%safe-div simp-sum (max 1 edge-n)))
         (inter-ratio (%safe-div inter (max 1 edge-n)))
         (occam (%occam-pass-rate))
         (global (%policy-weighted-sum
                  `(("global/simplicity-weight" 0.45 ,simp)
                    ("global/occam-weight" 0.30 ,occam)
                    ("global/interdisciplinary-weight" 0.25 ,inter-ratio)))))
    (list :simplicity simp
          :interdisciplinary-ratio inter-ratio
          :occam-pass-rate occam
          :global-score (%clamp global 0.0 1.0))))

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
  (let ((nodes (getf map :concept-nodes)))
    (if (null nodes) ""
        (getf (reduce (lambda (a b)
                        (if (> (getf b :count) (getf a :count)) b a))
                      nodes)
              :concept))))

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
         (simp-sum (reduce (lambda (acc e) (+ acc (%edge-simplicity e idx)))
                           local-edges :initial-value 0.0))
         (cross (count-if (lambda (e) (getf e :interdisciplinary)) local-edges))
         (simp (%safe-div simp-sum (max 1 n)))
         (cross-ratio (%safe-div cross (max 1 n))))
    (list :focus focus
          :edge-count n
          :simplicity simp
          :interdisciplinary-ratio cross-ratio
          :local-score (%policy-weighted-sum
                        `(("local/simplicity-weight" 0.6 ,simp)
                          ("local/interdisciplinary-weight" 0.4 ,cross-ratio))))))

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
         (aggression (signalograd-adjust-aggression
                      (%clamp (* (- 1.0 chaos-risk) (+ ag-base (* ag-scale x2))) ag-min ag-max)
                      runtime)))
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
          :balance (%policy-weighted-sum
                    `(("complexity/simple-weight" 0.5 ,simple-score)
                      ("complexity/possible-weight" 0.5 ,possible-score))))))

(defun %tool-edge-p (e)
  "True when edge E touches a tool/infrastructure concept."
  (let ((a (getf e :a)) (b (getf e :b)))
    (or (search "tool" a :test #'char-equal)
        (search "tool" b :test #'char-equal)
        (member a '("provider-router" "workspace" "memory") :test #'string=)
        (member b '("provider-router" "workspace" "memory") :test #'string=))))

(defun %coherent-edge-p (e)
  "True when edge E touches a memory or skill concept."
  (let ((a (getf e :a)) (b (getf e :b)))
    (or (search "memory" a :test #'char-equal)
        (search "memory" b :test #'char-equal)
        (search "skill" a :test #'char-equal)
        (search "skill" b :test #'char-equal))))

(defun %tool-memory-coherence (map)
  (let* ((tool-edges (remove-if-not #'%tool-edge-p (getf map :concept-edges)))
         (total (length tool-edges))
         (coherent (count-if #'%coherent-edge-p tool-edges)))
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
  "Vitruvian triad: strength × utility × beauty. ALL weights from harmony-policy (signalograd-adaptive)."
  (let* ((strength (%policy-weighted-sum
                    `(("vitruvian/strength-chaos-weight" 0.6 ,(- 1.0 (getf logistic :chaos-risk)))
                      ("vitruvian/strength-bounded-weight" 0.4 ,(getf lorenz :bounded-score)))))
         (utility (%policy-weighted-sum
                   `(("vitruvian/utility-global-weight" 0.35 ,(getf global :global-score))
                     ("vitruvian/utility-coherence-weight" 0.25 ,(%tool-memory-coherence map))
                     ("vitruvian/utility-balance-weight" 0.20 ,(getf (%complexity-balance map) :balance))
                     ("vitruvian/utility-supervision-weight" 0.20 ,(%supervision-rate)))))
         (beauty (%policy-weighted-sum
                  `(("vitruvian/beauty-ratio-weight" 0.50 ,(getf projection :ratio))
                    ("vitruvian/beauty-inter-weight" 0.25 ,(getf local :interdisciplinary-ratio))
                    ("vitruvian/beauty-simplicity-weight" 0.25 ,(getf global :simplicity)))))
         (signal (%policy-weighted-sum
                  `(("vitruvian/signal-strength-weight" 0.34 ,strength)
                    ("vitruvian/signal-utility-weight" 0.33 ,utility)
                    ("vitruvian/signal-beauty-weight" 0.33 ,beauty))))
                         0.0 1.0))
         (noise (- 1.0 signal)))
    (signalograd-adjust-vitruvian
     (list :strength strength :utility utility :beauty beauty :signal signal :noise noise)
     *runtime*)))

;;; --- Phase handlers ---
;;; Each takes (runtime ctx) and returns (values new-ctx next-phase).
;;; No setf on runtime — the dispatcher owns all mutation.

(defun %phase-observe (runtime ctx)
  "Phase 1: Snapshot concept graph, push to memory field."
  (declare (ignore ctx))
  (let ((new-ctx (list :map (%denoise-map (memory-map-sexp :entry-limit 120 :edge-limit 160))
                       :cycle (runtime-state-cycle runtime))))
    (when (and (fboundp 'memory-field-port-ready-p)
               (funcall 'memory-field-port-ready-p))
      (handler-case (funcall 'memory-field-load-graph) (error () nil)))
    (values new-ctx :evaluate-global)))

(defun %phase-evaluate-global (runtime ctx)
  "Phase 2: Global harmony score."
  (declare (ignore runtime))
  (let ((global (%global-harmony (getf ctx :map))))
    (values (append (list :global global) ctx)
            :evaluate-local)))

(defun %phase-evaluate-local (runtime ctx)
  "Phase 3: Local fractal harmony around best focus concept."
  (declare (ignore runtime))
  (let* ((map (getf ctx :map))
         (focus (%best-focus-concept map))
         (local (%local-fractal-harmony map focus)))
    (values (append (list :local local) ctx)
            :logistic-balance)))

(defun %phase-logistic-balance (runtime ctx)
  "Phase 4: Step logistic map, compute chaos risk and aggression."
  (let ((logistic (%step-logistic runtime)))
    (values (append (list :logistic logistic) ctx)
            :lambdoma-project)))

(defun %phase-lambdoma-project (runtime ctx)
  "Phase 5: Lambdoma convergence. If highly convergent AND low chaos,
   perform PLAGAL CADENCE (S→R): skip dominant, resolve directly to stabilize.
   Otherwise proceed to dominant function (:attractor-sync)."
  (declare (ignore runtime))
  (let* ((projection (%lambdoma-convergence (getf ctx :global) (getf ctx :local)))
         (convergent-p (getf projection :convergent-p))
         (chaos (getf (getf ctx :logistic) :chaos-risk))
         (plagal-threshold (harmony-policy-number "cadence/plagal-chaos-max" 0.15))
         (plagal-p (and convergent-p (< (or chaos 1.0) plagal-threshold))))
    (values (append (list :projection projection :plagal-cadence plagal-p) ctx)
            (if plagal-p :stabilize :attractor-sync))))

(defun %phase-attractor-sync (runtime ctx)
  "Phase 6: Step Lorenz + memory-field attractors."
  (let* ((plan (getf ctx :plan))
         (vitruvian (and plan (getf plan :vitruvian)))
         (sig (or (and vitruvian (getf vitruvian :signal)) 0.5))
         (noi (or (and vitruvian (getf vitruvian :noise)) 0.5))
         (lorenz (%step-lorenz runtime))
         (field-basin (when (and (fboundp 'memory-field-port-ready-p)
                                 (funcall 'memory-field-port-ready-p))
                        (handler-case

                            (funcall 'memory-field-step-attractors :signal sig :noise noi)

                          (error () nil))
                        (handler-case (funcall 'memory-field-basin-status) (error () nil)))))
    (values (append (list :lorenz lorenz :field-basin field-basin) ctx)
            :rewrite-plan)))

(defun %phase-rewrite-plan (runtime ctx)
  "Phase 7: Compute Vitruvian scores, decide rewrite readiness."
  (let* ((projection (getf ctx :projection))
         (logistic (getf ctx :logistic))
         (global (getf ctx :global))
         (local (getf ctx :local))
         (map (getf ctx :map))
         (lorenz (getf ctx :lorenz))
         (vitruvian (%vitruvian-scores global local projection logistic lorenz map))
         (ok (and (getf projection :convergent-p)
                  (< (getf logistic :chaos-risk)
                     (signalograd-effective-harmony-number "rewrite-plan/chaos-max" 0.55 runtime))
                  (>= (getf vitruvian :signal)
                      (signalograd-effective-harmony-number "rewrite-plan/signal-min" 0.62 runtime))))
         (plan (list :state-machine :harmonic
                     :ready ok
                     :focus (getf (getf ctx :local) :focus)
                     :aggression (getf logistic :rewrite-aggression)
                     :lambdoma-ratio (getf projection :ratio)
                     :vitruvian vitruvian
                     :dna-laws (getf *dna* :laws))))
    (when ok
      (incf (runtime-state-rewrite-count runtime)))
    (when (%trace-level-p :verbose)
      (trace-event "rewrite-decision" :chain
                   :metadata (list :ready ok
                                   :convergent (getf projection :convergent-p)
                                   :chaos-risk (getf logistic :chaos-risk)
                                   :signal-score (getf vitruvian :signal)
                                   :aggression (getf logistic :rewrite-aggression))))
    (runtime-log runtime :harmonic-rewrite-plan plan)
    (values (append (list :plan plan) ctx)
            :security-audit)))

(defun %phase-security-audit (runtime ctx)
  "Phase 8: Scan injection counts, update security posture."
  (let* ((event-count *security-event-count*)
         (entries (loop for k being the hash-keys of *security-injection-counts*
                        using (hash-value v) collect (cons k v)))
         (injection-total (reduce #'+ entries :key #'cdr :initial-value 0))
         (worst (reduce (lambda (a b) (if (> (cdr b) (cdr a)) b a))
                        entries :initial-value (cons nil 0)))
         (worst-frontend (car worst))
         (new-posture (cond
                        ((> injection-total 20) :alert)
                        ((> injection-total 5)  :elevated)
                        (t :nominal))))
    ;; Side effects: reset global security counters for next cycle.
    (setf *security-posture* new-posture)
    (setf *security-event-count* 0)
    (clrhash *security-injection-counts*)
    (when (%trace-level-p :verbose)
      (trace-event "security-audit" :chain
                   :metadata (list :posture new-posture
                                   :events-count event-count
                                   :injection-total injection-total)))
    (runtime-log runtime :security-audit
                 (list :events event-count
                       :injection-total injection-total
                       :worst-frontend worst-frontend
                       :posture new-posture))
    ;; DECEPTIVE CADENCE (D→T): If posture is :alert, dominant tension
    ;; resolves deceptively back to tonic (re-observe) instead of stabilize.
    ;; Like V→vi in music — expected resolution avoided, phrase extended.
    (let ((next-phase (if (eq new-posture :alert) :observe :stabilize)))
      (values (append (list :security (list :events event-count :posture new-posture)
                            :deceptive-cadence (eq new-posture :alert))
                      ctx)
              next-phase))))

(defun %phase-stabilize (runtime ctx)
  "Phase 9: Persist chronicles, dispatch reflection, cycle routing rules."
  (let ((ctx (if (and (fboundp 'memory-field-port-ready-p)
                      (funcall 'memory-field-port-ready-p))
                 (let ((basin (handler-case (funcall 'memory-field-basin-status) (error () nil))))
                   (if basin (append (list :field-basin basin) ctx) ctx))
                 ctx)))
    (runtime-log runtime :harmonic-stabilized
                 (list :phase :stabilize
                       :rewrite-count (runtime-state-rewrite-count runtime)))
    (when (%trace-level-p :verbose)
      (let* ((plan (getf ctx :plan))
             (vit (and plan (getf plan :vitruvian)))
             (logistic (getf ctx :logistic))
             (security (getf ctx :security)))
        (trace-event "harmonic-cycle" :chain
                     :metadata (list :cycle (or (getf ctx :cycle) 0)
                                     :phase :stabilize
                                     :strength (and vit (getf vit :strength))
                                     :utility (and vit (getf vit :utility))
                                     :beauty (and vit (getf vit :beauty))
                                     :signal (and vit (getf vit :signal))
                                     :chaos-risk (and logistic (getf logistic :chaos-risk))
                                     :rewrite-ready (and plan (getf plan :ready))
                                     :security-posture (and security (getf security :posture))))))
    (handler-case (chronicle-record-harmonic ctx) (error () nil))
    (handler-case (chronicle-record-graph-snapshot) (error () nil))
    (handler-case (signalograd-dispatch-reflection ctx :runtime runtime) (error () nil))
    (handler-case (%maybe-rewrite-routing-rules ctx) (error () nil))
    (values ctx :observe)))

;;; --- Dispatcher ---

(defun harmonic-state-step (&key (runtime *runtime*))
  "Step the harmonic state machine one phase. Pure dispatch with tonal function
   awareness (Funktionstheorie). Tracks cadence types: authentic (D→R),
   plagal (S→R), deceptive (D→T). Logs retrogression (D→S) as harmonic warning."
  (let* ((phase (runtime-state-harmonic-phase runtime))
         (ctx (or (runtime-state-harmonic-context runtime) '()))
         (entry (assoc phase *harmonic-phases*)))
    (if entry
        (multiple-value-bind (new-ctx next-phase)
            (funcall (cdr entry) runtime ctx)
          ;; Tonal function analysis: classify the cadence of this transition.
          (let* ((from-fn (%tonal-function phase))
                 (to-fn (%tonal-function next-phase))
                 (cadence (%cadence-type from-fn to-fn)))
            (when (eq cadence :retrogression)
              (%log :warn "harmonic" "Retrogression ~A→~A (~A→~A) — harmonic tension unresolved"
                    phase next-phase from-fn to-fn))
            ;; Annotate context with tonal function metadata for signalograd.
            (setf (runtime-state-harmonic-context runtime)
                  (append (list :tonal-function to-fn :cadence cadence) new-ctx))
            (setf (runtime-state-harmonic-phase runtime) next-phase)))
        (progn
          (%log :warn "harmonic" "Unknown phase: ~A, resetting to :observe" phase)
          (setf (runtime-state-harmonic-phase runtime) :observe)))
    runtime))

;;; --- Routing rules self-rewriting ---

(defun %maybe-rewrite-routing-rules (ctx)
  "Mutate routing rules based on accumulated experience.
   Only triggers when the harmonic plan is ready (convergent, low chaos, high vitruvian)
   and enough samples exist. Signalograd evolution_aggression_bias controls frequency."
  (let ((plan (getf ctx :plan)))
    (when (and plan (getf plan :ready))
      (let ((scores (handler-case
     (when (fboundp '%load-swarm-scores)
                        (%load-swarm-scores)
   (error () nil)))))
        (when (and scores (> (length scores) 10))
          ;; Ban models with <50% success after 5+ samples
          (dolist (s scores)
            (when (and (>= (or (getf s :samples) 0) 5)
                       (< (or (getf s :success-rate) 1.0) 0.5))
              (let ((model-id (getf s :model-id)))
                (unless (member model-id (getf *routing-rules-sexp* :model-bans)
                                :test #'string=)
                  (push model-id (getf *routing-rules-sexp* :model-bans))))))
          ;; Persist rules via model-policy-save
          (handler-case (model-policy-save) (error () nil)))))))
