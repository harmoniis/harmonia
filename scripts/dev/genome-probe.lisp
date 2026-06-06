;;;; genome-probe.lisp — the agent as a living organism: genome completeness, gene
;;;; integrity, germline-bounds-the-soma, immune RESPONSE (without autoimmunity), and
;;;; epigenetic heritability. Mostly PURE — runs fast WITHOUT a running runtime:
;;;;   sbcl --load src/core/boot.lisp --load scripts/dev/genome-probe.lisp \
;;;;        --eval '(harmonia::genome-probe-run)'

(in-package :harmonia)

(defparameter *gp-pass* 0)
(defparameter *gp-fail* 0)
(defun %g (label ok)
  (if ok (progn (incf *gp-pass*) (format t "  [PASS] ~A~%" label))
         (progn (incf *gp-fail*) (format t "  [FAIL] ~A~%" label))))
(defun %gpp (label thunk)
  (handler-case (let ((r (funcall thunk)))
                  (format t "~&[PROBE] ~A => ~A~%" label
                          (let ((s (princ-to-string r))) (if (> (length s) 200) (subseq s 0 200) s)))
                  r)
    (error (c) (format t "~&[PROBE] ~A => ERROR ~A~%" label c) nil)))

(defun genome-probe-run ()
  (setf *gp-pass* 0 *gp-fail* 0)
  (format t "~%════════ HARMONIA GENOME / LIVING-ORGANISM PROBE ════════~%")

  ;; ── A. GENOME COMPLETENESS (the perfect genome is WHOLE) ──
  (format t "~%── A. GENOME COMPLETENESS ──~%")
  (%g "genome has IDENTITY (creator+pgp)" (equal (getf (getf *dna* :creator) :pgp) "88E016462EFF9672"))
  (%g "PRIME-DIRECTIVE present AND simple (one concise line, no prose flooding)"
      (and (dna-prime-directive) (plusp (length (dna-prime-directive))) (< (length (dna-prime-directive)) 160)))
  (%g "the hard laws ARE the :constraints (code, not prose) — non-empty"
      (and (consp (dna-laws)) (>= (length (dna-laws)) 5)))
  (%g "genome has GENES, BOUNDS, FOUNDATION, IMMUNE"
      (and (getf *dna* :genes) (getf *dna* :bounds) (getf *dna* :foundation) (getf *dna* :immune)))
  ;; B. GENE INTEGRITY — every gene resolves to a real function (no dead mapping)
  (%gpp "genes" (lambda () (getf *dna* :genes)))
  (%g "dna-valid-p: identity + completeness + every gene fboundp" (dna-valid-p))

  ;; ── C. GERMLINE BOUNDS THE SOMA (clamp-at-write, from the genome) ──
  (format t "~%── C. GERMLINE BOUNDS THE SOMA ──~%")
  (let ((ceiling (cdr (dna-bound :concept-edge-weight))))
    (%g "meditation ceiling comes from the genome :bounds" (and (numberp ceiling) (> ceiling 0)))
    ;; push reinforcement far past the ceiling — it must clamp to the genome bound
    (let ((w (%reinforce-weight 1000.0 1000.0)))
      (%gpp "reinforce 1000+1000 (clamped)" (lambda () w))
      (%g "epigenetic mark is clamped to the germline bound at the write site"
          (<= w (+ ceiling 1d-6))))
    ;; a value within bounds is unchanged (no over-clamping)
    (%g "reinforcement within bounds grows normally (not flattened)"
        (= (%reinforce-weight 1.0 2.0) 3.0)))

  ;; ── D. IMMUNE RESPONSE (posture gates behavior) ──
  (format t "~%── D. IMMUNE RESPONSE ──~%")
  (let ((nominal (dna-immune-response :nominal))
        (elevated (dna-immune-response :elevated))
        (alert (dna-immune-response :alert)))
    (%gpp "posture map (nominal/elevated/alert chaos-max)"
          (lambda () (list (getf nominal :chaos-risk-max) (getf elevated :chaos-risk-max) (getf alert :chaos-risk-max))))
    (%g "escalation MONOTONICALLY tightens chaos tolerance (nominal > elevated > alert)"
        (> (getf nominal :chaos-risk-max) (getf elevated :chaos-risk-max) (getf alert :chaos-risk-max)))
    (%g "escalation reduces swarm fan-out"
        (>= (getf nominal :swarm-fanout) (getf elevated :swarm-fanout) (getf alert :swarm-fanout)))
    (%g ":alert refuses risky capabilities (exec, datamine)"
        (and (null (getf alert :allow-exec)) (null (getf alert :allow-datamine)))))
  ;; the live gates respond to the current posture
  (let ((*security-posture* :alert))
    (%g "immune-allows-p :exec is NIL under :alert" (not (immune-allows-p :exec)))
    (%g "immune-allows-p :datamine is NIL under :alert" (not (immune-allows-p :datamine)))
    (%g "immune-gated :chaos-risk-max tightens under :alert (≤ 0.30)" (<= (immune-gated :chaos-risk-max 0.55) 0.30)))

  ;; ── E. NO AUTOIMMUNITY (a healthy organism is never strangled) ──
  (format t "~%── E. NO AUTOIMMUNITY ──~%")
  (let ((*security-posture* :nominal))
    (%g "under :nominal, exec is ALLOWED (legitimate work flows)" (immune-allows-p :exec))
    (%g "under :nominal, datamine is ALLOWED" (immune-allows-p :datamine))
    (%g "under :nominal, chaos-max equals the baseline (no restriction)"
        (= (immune-gated :chaos-risk-max 0.55) 0.55))
    (%g "under :nominal, swarm fan-out equals the baseline" (= (immune-gated :swarm-fanout 3) 3)))
  (let ((*security-posture* :elevated))
    ;; elevated must still let legitimate work through — only :alert is defensive
    (%g "under :elevated, exec STILL flows (graduated, not paralysed)" (immune-allows-p :exec))
    (%g "under :elevated, chaos-max is tightened but non-zero (still acts)"
        (> (immune-gated :chaos-risk-max 0.55) 0.0)))

  ;; ── F. EPIGENETIC HERITABILITY (fluency survives 'restart') ──
  (format t "~%── F. HERITABILITY: REPL FLUENCY ──~%")
  (let ((m (format nil "genome-probe-model-~36R" (get-universal-time))))
    (%record-repl-perf m :code-ok) (%record-repl-perf m :code-ok) (%record-repl-perf m :code-error)
    (let ((f-before (%repl-fluency m)))
      (%save-repl-fluency)
      (remhash m *repl-model-perf*)                          ; simulate restart (lose RAM)
      (%g "fluency lost after clearing RAM" (null (gethash m *repl-model-perf*)))
      (%load-repl-fluency)                                   ; inherit from disk
      (let ((f-after (%repl-fluency m)))
        (%gpp "fluency before/after restart" (lambda () (list :before f-before :after f-after)))
        (%g "REPL fluency is HERITABLE (survives restart, steers selection)"
            (and (realp f-before) (realp f-after) (= f-before f-after))))))

  ;; ── G. THE GENOME CONTROLS THE LLM — via the MEMORY MAP, not prompt flooding ──
  (format t "~%── G. GENOME → LLM (via memory map) ──~%")
  (let ((map (memory-map-sexp :entry-limit 3 :edge-limit 3)))
    (%g "system prompt stays MINIMAL (no prose flooding — protects the free model)"
        (< (length (dna-system-prompt)) 60))
    (%g "the genome reaches the LLM via the memory map (:dna carries prime-directive + laws)"
        (let ((dna (getf map :dna)))
          (and dna (getf dna :prime-directive) (getf dna :laws)))))

  (format t "~%════════ GENOME PROBE: ~A pass / ~A fail ════════~%" *gp-pass* *gp-fail*)
  (sb-ext:exit :code (if (zerop *gp-fail*) 0 1)))
