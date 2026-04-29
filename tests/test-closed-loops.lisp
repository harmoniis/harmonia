;;; test-closed-loops.lisp — Deterministic tests for the dynamics closed-loop wires.
;;;
;;; Verifies the four loops that were unwired before the systemic fix:
;;;   1. Signalograd projection broadcast and apply.
;;;   2. Memory-field basin / dream broadcast handlers.
;;;   3. Harmony :logistic-r-delta clamp via sanitize-proposal.
;;;   4. %step-logistic actually moves runtime-state-harmonic-r.
;;;
;;; Runs without LLM, without IPC, without Rust runtime — pure Lisp determinism.

(in-package :harmonia)

(unless (boundp '*test-pass*) (defparameter *test-pass* 0))
(unless (boundp '*test-fail*) (defparameter *test-fail* 0))

(defmacro closed-loop-assert (name expr)
  `(handler-case
       (if ,expr
           (progn (incf *test-pass*) (format t "  ✓ ~A~%" ,name))
           (progn (incf *test-fail*) (format t "  ✗ ~A — assertion failed~%" ,name)))
     (error (c)
       (incf *test-fail*) (format t "  ✗ ~A — error: ~A~%" ,name c))))

(defun %close-to (a b &optional (eps 1.0e-6))
  (< (abs (- a b)) eps))

(defun run-closed-loop-tests ()
  (setf *test-pass* 0 *test-fail* 0)
  (format t "~%═══ CLOSED-LOOP WIRING TESTS ══════════════════════════~%")

  ;; ── Sanitize-proposal: logistic-r-delta clamp ─────────────────────
  (format t "~%── signalograd sanitize: logistic-r-delta ──~%")

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :confidence 0.5
                      :harmony (:logistic-r-delta 0.5)))))
    (closed-loop-assert
        "logistic-r-delta clamped to harmony/logistic-r-delta-max (default 0.02)"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.02)))

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :harmony (:logistic-r-delta -0.7)))))
    (closed-loop-assert "negative delta clamped symmetrically to -0.02"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) -0.02)))

  (let ((sanitized (%signalograd-sanitize-proposal
                    '(:cycle 1 :harmony (:logistic-r-delta 0.005)))))
    (closed-loop-assert "in-range delta passes through"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.005)))

  (let ((sanitized (%signalograd-sanitize-proposal '(:cycle 1))))
    (closed-loop-assert "missing harmony section yields zero delta"
        (%close-to (getf (getf sanitized :harmony) :logistic-r-delta) 0.0)))

  ;; ── signalograd-apply-proposal end-to-end ─────────────────────────
  (format t "~%── signalograd-apply-proposal stores projection ──~%")

  (let* ((rt (make-runtime-state-fresh))
         (proposal '(:signalograd-proposal :cycle 17 :confidence 0.62
                     :stability 0.5 :novelty 0.3
                     :harmony (:logistic-r-delta 0.011))))
    (signalograd-apply-proposal proposal :runtime rt)
    (closed-loop-assert "projection stored after apply"
        (and (listp (runtime-state-signalograd-projection rt))
             (= 17 (getf (runtime-state-signalograd-projection rt) :cycle))))
    (closed-loop-assert "signalograd-logistic-r-delta reads from runtime"
        (%close-to (signalograd-logistic-r-delta rt) 0.011)))

  (let ((rt (make-runtime-state-fresh)))
    (closed-loop-assert "signalograd-logistic-r-delta defaults to 0 with no projection"
        (%close-to (signalograd-logistic-r-delta rt) 0.0)))

  ;; ── %step-logistic actually moves r ───────────────────────────────
  (format t "~%── %step-logistic applies r delta ──~%")

  (let ((rt (make-runtime-state-fresh)))
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta 0.01))
     :runtime rt)
    (let ((r0 (runtime-state-harmonic-r rt)))
      (let ((*runtime* rt)) (%step-logistic rt))
      (closed-loop-assert "r moved by the applied delta"
          (%close-to (runtime-state-harmonic-r rt) (+ r0 0.01)))))

  (let ((rt (make-runtime-state-fresh)))
    ;; No projection applied → delta = 0 → r unchanged.
    (let ((r0 (runtime-state-harmonic-r rt)))
      (let ((*runtime* rt)) (%step-logistic rt))
      (closed-loop-assert "r unchanged when no projection is present"
          (%close-to (runtime-state-harmonic-r rt) r0))))

  (let ((rt (make-runtime-state-fresh)))
    ;; Drive r toward the upper edge with many positive deltas; verify clamp.
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta 0.02))
     :runtime rt)
    (loop repeat 200 do (let ((*runtime* rt)) (%step-logistic rt)))
    (let ((edge 3.56995) (window 0.4))
      (closed-loop-assert "r clamped to upper bound (edge + window)"
          (%close-to (runtime-state-harmonic-r rt) (+ edge window)))))

  (let ((rt (make-runtime-state-fresh)))
    (signalograd-apply-proposal
     '(:signalograd-proposal :cycle 1 :harmony (:logistic-r-delta -0.02))
     :runtime rt)
    (loop repeat 200 do (let ((*runtime* rt)) (%step-logistic rt)))
    (let ((edge 3.56995) (window 0.4))
      (closed-loop-assert "r clamped to lower bound (edge - window)"
          (%close-to (runtime-state-harmonic-r rt) (- edge window)))))

  ;; ── Memory-field signals plumbed into observation packet ────────
  (format t "~%── observation packet carries field + palace signals ──~%")

  (closed-loop-assert "field-stability is 0 with no basin info"
      (%close-to (%signalograd-field-stability '()) 0.0))

  (closed-loop-assert "field-stability saturates as dwell grows"
      (let ((s (%signalograd-field-stability
                '(:field-basin (:dwell-ticks 200)))))
        (and (> s 0.85) (<= s 1.0))))

  (closed-loop-assert "field-stability mid-range at dwell=20"
      (%close-to (%signalograd-field-stability
                  '(:field-basin (:dwell-ticks 20))) 0.5 0.01))

  (closed-loop-assert "palace-density returns 0 when port offline"
      (%close-to (%signalograd-palace-density) 0.0))

  (let* ((rt (make-runtime-state-fresh))
         (ctx '(:cycle 1
                :field-basin (:dwell-ticks 60)
                :map (:concept-nodes nil :concept-edges nil)))
         (sexp (%signalograd-observation-sexp ctx rt)))
    (closed-loop-assert ":field-basin-stability appears in observation sexp"
        (search ":field-basin-stability" sexp))
    (closed-loop-assert ":field-recall-strength appears in observation sexp"
        (search ":field-recall-strength" sexp))
    (closed-loop-assert ":field-eigenmode-coherence appears in observation sexp"
        (search ":field-eigenmode-coherence" sexp))
    (closed-loop-assert ":datamine-success-rate appears in observation sexp"
        (search ":datamine-success-rate" sexp))
    (closed-loop-assert ":datamine-avg-latency appears in observation sexp"
        (search ":datamine-avg-latency" sexp))
    (closed-loop-assert ":palace-graph-density appears in observation sexp"
        (search ":palace-graph-density" sexp)))

  (closed-loop-assert "field-coherence is 0 when port offline"
      (%close-to (%signalograd-field-coherence) 0.0))

  (closed-loop-assert "datamine-stats returns plist with both keys when offline"
      (let ((s (%signalograd-datamine-stats)))
        (and (%close-to (getf s :success-rate) 0.0)
             (%close-to (getf s :avg-latency-ms) 0.0))))

  ;; ── Chaos-risk varies as r evolves ────────────────────────────────
  (format t "~%── chaos-risk varies with r ──~%")

  (let ((rt-near (make-runtime-state-fresh))
        (rt-far  (make-runtime-state-fresh)))
    (setf (runtime-state-harmonic-r rt-near) 3.55) ; near edge
    (setf (runtime-state-harmonic-r rt-far)  3.20) ; far from edge
    (let ((cr-near (let ((*runtime* rt-near)) (getf (%step-logistic rt-near) :chaos-risk)))
          (cr-far  (let ((*runtime* rt-far))  (getf (%step-logistic rt-far)  :chaos-risk))))
      (closed-loop-assert "chaos-risk near edge is high"
          (> cr-near 0.9))
      (closed-loop-assert "chaos-risk far from edge is low"
          (< cr-far 0.1))))

  (format t "~%───────────────────────────────────────────────────────~%")
  (format t "PASS: ~D    FAIL: ~D~%" *test-pass* *test-fail*)
  (values *test-pass* *test-fail*))

(defun make-runtime-state-fresh ()
  "Construct a minimal runtime-state for deterministic test runs."
  (make-runtime-state))
