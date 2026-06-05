;;;; cognition-probe.lisp — QUALITY (not liveness) of the cognitive machinery:
;;;; recursive self-improvement (meditation/dream/evolution/signalograd), memory
;;;; classification, eval harnessing/steering, orchestration, state machines, matrix.
;;;;
;;;; Run headless against a LIVE runtime, AFTER loading deep-probe.lisp (reuses its
;;;; helpers %pp/%assert/%reply-plist/%num):
;;;;   sbcl --load src/core/boot.lisp --eval '(harmonia:start :run-loop nil)' \
;;;;        --load scripts/dev/deep-probe.lisp --load scripts/dev/cognition-probe.lisp \
;;;;        --eval '(harmonia::cognition-probe-run)'
;;;;
;;;; [PASS]/[FAIL] are hard invariants; [GAP] are honest quality findings that do
;;;; not fail the run (the point is to KNOW what works and what is weak).

(in-package :harmonia)

(defparameter *cog-gap* 0)
(defun %gap (label) (incf *cog-gap*) (format t "  [GAP]  ~A~%" label))

(defun %uniq (stem) (format nil "~A-~36R" stem (get-universal-time)))

(defun cognition-probe-run ()
  (setf *probe-pass* 0 *probe-fail* 0 *cog-gap* 0)
  (format t "~%════════ HARMONIA COGNITION QUALITY PROBE ════════~%")

  ;; ─────────────────────── A. MEMORY CLASSIFICATION ───────────────────────
  (format t "~%── A. MEMORY CLASSIFICATION QUALITY ──~%")
  ;; A1: domain map accurate on its KNOWN vocabulary.
  (let* ((known '(("harmony" . :music) ("melody" . :music)
                  ("geometry" . :math) ("fractal" . :math)
                  ("rust" . :engineering) ("api" . :engineering)
                  ("dream" . :cognitive) ("dna" . :cognitive)))
         (hits (count-if (lambda (p) (eq (%concept-domain (car p)) (cdr p))) known)))
    (%pp "domain map (known vocab)"
         (lambda () (mapcar (lambda (p) (cons (car p) (%concept-domain (car p)))) known)))
    (%assert (format nil "domain classification accurate on known vocab (~D/~D)" hits (length known))
             (>= hits (1- (length known)))))
  ;; A2 (W3): the agent's technical vocabulary must classify into the fixed domains,
  ;; not collapse to :generic — and only into the 7 the Rust enum knows (Lisp↔Rust parity).
  (let* ((tech '("rk4" "lorenz" "attractor" "ractor" "ipc" "eigenvalue" "laplacian"
                 "spectral" "actor" "sexp" "chronicle" "signalograd"))
         (seven '(:music :math :engineering :cognitive :life :system :generic))
         (doms (mapcar #'%concept-domain tech))
         (generic (count :generic doms))
         (math-or-eng (count-if (lambda (d) (member d '(:math :engineering) :test #'eq)) doms)))
    (%pp "domain map (technical vocab)" (lambda () (mapcar #'cons tech doms)))
    (%assert "technical vocab no longer collapses to :generic (≤2 of 12)" (<= generic 2))
    (%assert "technical vocab lands in math/engineering (the right domains)" (>= math-or-eng 8))
    (%assert "every domain is one of the fixed 7 (Lisp↔Rust parity)"
             (every (lambda (d) (member d seven :test #'eq)) doms)))

  ;; ─────────────────────── B. EVAL HARNESSING / STEERING ───────────────────────
  (format t "~%── B. EVAL HARNESSING (does eval steer the model?) ──~%")
  ;; B1: code-errors demote a model's REPL fluency.
  (let ((m (%uniq "cogp-model")))
    (dotimes (i 3) (%record-repl-perf m :code-ok))
    (let ((f0 (%repl-fluency m)))
      (dotimes (i 8) (%record-repl-perf m :code-error))
      (let ((f1 (%repl-fluency m)))
        (%pp "fluency before/after 8 errors" (lambda () (list :before f0 :after f1)))
        (%assert "code-errors demote model fluency (f1 < f0)"
                 (and (realp f0) (realp f1) (< f1 f0)))
        (%assert "fluency collapses below 0.5 after error burst"
                 (and (realp f1) (< f1 0.5))))))
  ;; B2: %task-kind classification on a matrix (report actuals; assert the clear ones).
  (let* ((cases '(("implement a REST endpoint in Rust" :software-dev :coding)
                  ("fix the null pointer bug"          :software-dev :coding)
                  ("how does the orchestrator work?"   :general)
                  ("summarize yesterday's notes"       :memory-ops)))
         (results (mapcar (lambda (c) (list (first c) :got (%task-kind (first c)) :exp (rest c))) cases))
         (hits (count-if (lambda (c) (member (%task-kind (first c)) (rest c) :test #'eq)) cases)))
    (%pp "task-kind matrix" (lambda () results))
    (%assert (format nil "task-kind classification reasonable (~D/~D)" hits (length cases))
             (>= hits (1- (length cases)))))
  ;; B3: CLOSED LOOP — eval feedback actually steers selection.
  (let* ((prompt "implement a feature")
         (chosen (choose-model prompt)))
    (%pp "chosen model before error burst" (lambda () chosen))
    (if (and (stringp chosen) (> (length chosen) 0))
        (progn
          (dotimes (i 15) (%record-repl-perf chosen :code-error))
          (let ((after (choose-model prompt)) (fl (%repl-fluency chosen)))
            (%pp "chosen after 15 errors / its fluency" (lambda () (list :after after :fluency fl)))
            (%assert "eval feedback steers selection (model changes OR its fluency collapses)"
                     (or (not (equal after chosen)) (and (realp fl) (< fl 0.3))))))
        (%gap "choose-model returned no model id — cannot test eval→selection steering")))

  ;; ─────────────────────── C. RECURSIVE SELF-IMPROVEMENT ───────────────────────
  (format t "~%── C. RECURSIVE SELF-IMPROVEMENT ──~%")
  ;; C1 meditation: strengthen an existing edge (Hebbian boost).
  (let ((a (%uniq "cogp-a")) (b (%uniq "cogp-b")))
    (%upsert-concept-node a :daily 0 "cogp-1")
    (%upsert-concept-node b :daily 0 "cogp-1")
    (%upsert-concept-edge a b :cooccur)
    (let* ((k (%edge-key a b))
           (w0 (getf (gethash k *memory-concept-edges*) :weight)))
      (memory-meditate (list a b) :success t)
      (let ((w1 (getf (gethash k *memory-concept-edges*) :weight)))
        (%pp "meditation edge weight before/after" (lambda () (list :before w0 :after w1)))
        (%assert "meditation strengthens a co-activated edge by the learning rate"
                 (and (realp w0) (realp w1) (= (- w1 w0) *meditation-learning-rate*))))))
  ;; C2 meditation: bridge formation at the co-activation threshold.
  (let ((a (%uniq "cogp-c")) (b (%uniq "cogp-d")))
    (let ((k (%edge-key a b)))
      (remhash k *meditation-co-activation-log*)
      ;; threshold-1 co-activations must NOT bridge yet
      (dotimes (i (1- *meditation-bridge-threshold*)) (memory-meditate (list a b) :success t))
      (%assert "no bridge before co-activation threshold"
               (null (gethash k *memory-concept-edges*)))
      ;; the threshold-th co-activation bridges
      (memory-meditate (list a b) :success t)
      (let ((edge (gethash k *memory-concept-edges*)))
        (%pp "bridge edge after threshold" (lambda () edge))
        (%assert "meditation creates a :meditation bridge at threshold"
                 (and edge (member :meditation (getf edge :reasons) :test #'eq))))))
  ;; C3 meditation: success learns faster than failure (boost 2 vs 1).
  (let ((a (%uniq "cogp-e")) (b (%uniq "cogp-f")) (c (%uniq "cogp-g")) (d (%uniq "cogp-h")))
    (%upsert-concept-edge a b :cooccur) (%upsert-concept-edge c d :cooccur)
    (let* ((ka (%edge-key a b)) (kc (%edge-key c d))
           (wa0 (getf (gethash ka *memory-concept-edges*) :weight))
           (wc0 (getf (gethash kc *memory-concept-edges*) :weight)))
      (memory-meditate (list a b) :success t)
      (memory-meditate (list c d) :success nil)
      (let ((da (- (getf (gethash ka *memory-concept-edges*) :weight) wa0))
            (dc (- (getf (gethash kc *memory-concept-edges*) :weight) wc0)))
        (%pp "boost success vs failure" (lambda () (list :success da :failure dc)))
        (%assert "successful interactions strengthen more than failed ones" (> da dc)))))
  ;; C4 dream: field self-maintenance runs, reports stats, AND does not regress coherence.
  (let ((coh0 (%num (memory-field-eigenmode-status) :coherence)))
    (let ((d (%pp "memory-field dream" (lambda () (memory-field-dream)))))
      (%assert "dream returns prune/merge/crystallize stats"
               (let ((pl (%reply-plist d)))
                 (and (listp pl)
                      (or (getf pl :pruned) (getf pl :merged) (getf pl :crystallized)
                          (member :pruned pl) (member :merged pl) (member :crystallized pl))))))
    (let ((coh1 (%num (memory-field-eigenmode-status) :coherence)))
      (%pp "field coherence before/after dream" (lambda () (list :before coh0 :after coh1)))
      (if (and (realp coh0) (realp coh1))
          (%assert "dream does not regress field coherence (quality, not just liveness)"
                   (>= coh1 (- coh0 1d-6)))
          (%gap "field coherence not numeric (no recall yet) — dream coherence quality not measured this run"))))
  ;; C8 BOUNDED + VARIANCE-PRESERVING (W2): repeated meditation must converge to the
  ;; ceiling (no runaway), AND unequal reinforcement across a diverse set must keep weight
  ;; VARIANCE — flat weights degenerate the Laplacian spectrum just as runaway does. Ties
  ;; the meditation→snapshot→1B-warm-start→P1-spectrum coupling together.
  (let ((a (%uniq "cogp-sat-a")) (b (%uniq "cogp-sat-b")))
    (%upsert-concept-edge a b :cooccur)
    (let ((k (%edge-key a b)))
      (dotimes (i 50) (memory-meditate (list a b) :success t))
      (let ((w (getf (gethash k *memory-concept-edges*) :weight)))
        (%pp "edge weight after 50 meditations" (lambda () (list :w w :ceiling *concept-edge-weight-max*)))
        (%assert "meditation edge weight is BOUNDED by the ceiling (no runaway)"
                 (and (realp w) (<= w (+ *concept-edge-weight-max* 1d-6)))))))
  (let* ((cs (loop for i below 6 collect (%uniq (format nil "cogp-var-~D" i))))
         (ks (loop for i from 0 below (1- (length cs))
                   do (%upsert-concept-edge (nth i cs) (nth (1+ i) cs) :cooccur)
                   collect (%edge-key (nth i cs) (nth (1+ i) cs)))))
    (loop for i from 0 below (1- (length cs))           ; reinforce each pair a different amount
          do (dotimes (j (1+ i)) (memory-meditate (list (nth i cs) (nth (1+ i) cs)) :success t)))
    (let* ((ws (remove nil (mapcar (lambda (k) (getf (gethash k *memory-concept-edges*) :weight)) ks)))
           (spread (if ws (- (reduce #'max ws) (reduce #'min ws)) 0)))
      (%pp "diverse edge weights (variance check)" (lambda () (list :weights ws :spread spread)))
      (%assert "weights keep VARIANCE under unequal reinforcement (bounded, not flattened)"
               (> spread 0.0))))
  ;; C5 evolution: recording an outcome updates the model's running success-rate.
  (let ((m (%uniq "cogp-evo")))
    (handler-case
        (let ()
          (model-policy-record-outcome :model m :success t :latency-ms 100 :harmony-score 0.8 :cost-usd 0.0)
          (model-policy-record-outcome :model m :success t :latency-ms 100 :harmony-score 0.8 :cost-usd 0.0)
          (let* ((scores (%load-swarm-scores))
                 (entry (find m scores :key (lambda (e) (getf e :model-id)) :test #'equal)))
            (%pp "evolution swarm-score entry" (lambda () entry))
            (if entry
                (progn
                  (%assert "successes accrue samples + raise success-rate (evolution learns up)"
                           (and (>= (or (getf entry :samples) 0) 1)
                                (>= (or (getf entry :success-rate) 0) 0.5)))
                  ;; ...and failures pull it back DOWN (symmetric learning, not a ratchet).
                  (let ((up (getf entry :success-rate)))
                    (dotimes (i 4) (model-policy-record-outcome :model m :success nil :latency-ms 100 :cost-usd 0.0))
                    (let* ((e2 (find m (%load-swarm-scores) :key (lambda (e) (getf e :model-id)) :test #'equal))
                           (down (and e2 (getf e2 :success-rate))))
                      (%pp "success-rate up then down" (lambda () (list :after-success up :after-failures down)))
                      (%assert "failures pull success-rate back down (not a one-way ratchet)"
                               (and (realp down) (< down up))))))
                (%gap "model-policy-record-outcome did not surface in %load-swarm-scores for a fresh model — evolution recording may be scoped to known profiles only"))))
      (error (e) (%gap (format nil "evolution outcome path errored: ~A" e)))))
  ;; C6 signalograd: the kernel state is live, coherent, and non-decreasing across an
  ;; observation. The snapshot is a TAGGED list (:signalograd-snapshot :cycle N …), so
  ;; :cycle lives after the tag. (Attractor evolution over 25 steps is proven in deep-probe.)
  (flet ((sg-cycle ()
           (let ((pl (%reply-plist (ipc-call "(:component \"signalograd\" :op \"snapshot\")"))))
             (and (listp pl) (getf (cdr pl) :cycle)))))
    (let ((c0 (sg-cycle)))
      (ipc-call "(:component \"signalograd\" :op \"observe\" :observation (:signalograd-observe :cycle 1 :signal 0.8 :noise 0.1 :chaos-risk 0.3 :reward 0.7 :stability 0.7 :novelty 0.4))")
      (let ((c1 (sg-cycle)))
        (%pp "signalograd cycle before/after observe" (lambda () (list :before c0 :after c1)))
        (%assert "signalograd kernel state is live + coherent (cycle valid, non-decreasing)"
                 (and (integerp c0) (integerp c1) (>= c0 0) (>= c1 c0))))))
  ;; C7 signalograd → routing: routing weights are modulated (not static defaults).
  (let ((ws (handler-case
                (list :success (signalograd-routing-weight :success 0.20 *runtime*)
                      :price   (signalograd-routing-weight :price 0.35 *runtime*)
                      :reasoning (signalograd-routing-weight :reasoning 0.15 *runtime*))
              (error () nil))))
    (%pp "signalograd-modulated routing weights" (lambda () ws))
    (%assert "signalograd routing weights are live + clamped in [0.05,0.70]"
             (and ws (every (lambda (kv) (or (keywordp kv) (%in kv 0.05 0.70))) ws))))

  ;; ─────────────────────── D. STATE MACHINES ───────────────────────
  (format t "~%── D. STATE MACHINES ──~%")
  ;; D1 harmonic phase FSM: %next-phase walks all phases exactly once and cycles.
  (let* ((start (or (first *harmonic-phases*) :observe))
         (seq (loop with p = start repeat (length *harmonic-phases*)
                    collect p do (setf p (%next-phase p)))))
    (%pp "harmonic phase sequence" (lambda () seq))
    (%assert "harmonic FSM visits every phase exactly once"
             (null (set-difference *harmonic-phases* seq)))
    (%assert "harmonic FSM cycles (last phase → first)"
             (eq (%next-phase (car (last *harmonic-phases*))) start)))
  ;; D2 security posture: a valid state machine value is exposed.
  (%pp "security posture" (lambda () *security-posture*))
  (%assert "security posture is a valid FSM state"
           (member *security-posture* '(:nominal :elevated :alert) :test #'eq))

  ;; ─────────────────────── E. HARMONIC MATRIX ───────────────────────
  (format t "~%── E. HARMONIC MATRIX ──~%")
  ;; E1 observe-route updates matrix edge stats (Hebbian route memory).
  (let ((from (%uniq "cogp-src")) (to (%uniq "cogp-dst")))
    (handler-case
        (progn
          (harmonic-matrix-observe-route from to t 120 0.01)
          (let ((rep (princ-to-string (harmonic-matrix-report))))
            (%pp "matrix report (truncated)" (lambda () (subseq rep 0 (min 280 (length rep)))))
            (%assert "observing a route registers it in the matrix (uses recorded)"
                     (and (search from rep) (search to rep)))))
      (error (e) (%gap (format nil "harmonic-matrix observe/report errored: ~A" e)))))
  ;; E2 KNOWN OPEN LOOP: matrix route observations are recorded but not consumed by
  ;; model selection (%selection-chain-tiered does not gate on harmonic-matrix-route-allowed-p).
  (%gap "harmonic-matrix is an OPEN loop for model selection: routes are observed (edges/success-rate updated) but %selection-chain-tiered never consults harmonic-matrix-route-allowed-p, so matrix experience does not yet steer which model is picked. Wire it to close the loop.")

  (format t "~%════════ COGNITION TALLY: ~A pass / ~A fail / ~A gap ════════~%"
          *probe-pass* *probe-fail* *cog-gap*)
  (sb-ext:exit :code (if (zerop *probe-fail*) 0 1)))
