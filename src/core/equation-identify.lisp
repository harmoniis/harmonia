;;; equation-identify.lisp — Phase 8: the self-rewrite loop, parameter level.
;;;
;;; The defensible form of "the agent rewrites itself": extract the minimal-description-
;;; length governing law of a clean internal dynamic (the signalograd readout Θ·Ξ — already
;;; a linear functional over a NAMED basis, so it is exactly symbolically extractable),
;;; express it as an s-expression, EVAL it through the agent's own real sexp-eval, and use
;;; the result to nudge ONE bounded policy parameter. This is the constitutional
;;;   :reduce-kolmogorov-complexity  (Solomonoff prior exp(-size/40))   +
;;;   :path-of-minimum-action        (least-action descent, lifted to program space).
;;;
;;; It fills the harmonic machine's :rewrite-plan gated action (harmonic-machine.lisp): when
;;; the gate is open (lambdoma-convergent ∧ chaos-risk<0.55 ∧ vitruvian-signal≥0.62) the
;;; agent identifies its law and descends one parameter toward it.
;;;
;;; LIVE SEAM (env-blocked in a sandbox without runtime IPC): COEFFICIENTS are read from
;;; signalograd via IPC (the readout Θ for a head) and PARAM-AFTER is written to
;;; config-store. Both are passed in / returned here so the loop's LOGIC is verified offline
;;; against the real sexp-eval (tests/test-equation-identify.lisp).

(in-package :harmonia)

(defun %ei-clamp (x lo hi) (max lo (min hi x)))

(defun %ei-round (x &optional (places 4))
  "Round to PLACES decimals — minimal description, single-float result."
  (let ((scale (expt 10 places)))
    (/ (fround (* x scale)) (float scale 1.0))))

(defun %identify-readout-law (coefficients basis-names &key (threshold 1.0e-3))
  "Kolmogorov-minimal governing law as an s-expr: (+ (* c_i b_i) ...) over the NAMED basis,
dropping every |c_i| < THRESHOLD (occam — only terms that carry information survive).
BASIS-NAMES are s-expr atoms or forms (e.g. X or (SIN Y)). Exact by construction: no
regression, no affine gauge — this is reading Θ off a fixed named basis."
  (let ((terms '()))
    (loop for c in coefficients
          for b in basis-names
          when (>= (abs c) threshold)
            do (push (list '* (%ei-round c) b) terms))
    (setf terms (nreverse terms))
    (cond ((null terms) 0)
          ((= (length terms) 1) (first terms))
          (t (cons '+ terms)))))

(defun %law-description-length (law) (length (princ-to-string law)))

(defun %law-solomonoff-prior (law)
  "The constitution's universal prior over the law's description (compression.lisp:35)."
  (exp (- (/ (%law-description-length law) 40.0))))

(defun %eval-law (law bindings)
  "Evaluate a recovered governing law through the agent's OWN sandboxed sexp-eval (%reval).
BINDINGS is an alist of (symbol . number); primitives (+ * - sin cos …) come from the REPL
whitelist. This is the agent evaluating its own extracted equation — the heart of the vision."
  (%reval law bindings))

(defun %least-action-parameter (current target lo hi &key (rate 0.5))
  "One least-action step: move CURRENT toward TARGET, clamped to [LO,HI]. The recovered
law informs the parameter; the bound keeps the rewrite safe (never destroys the policy)."
  (%ei-clamp (+ current (* rate (- target current))) lo hi))

(defun harmonic-rewrite-descend (&key gate-open coefficients basis-names bindings
                                      (current 1.0) (lo 0.1) (hi 5.0) (rate 0.5))
  "The :rewrite-plan gated action — extract → s-expr → eval → bounded parameter.
Returns a plist describing the descent (or a no-op when the gate is closed). The agent
extracts its readout law, evaluates it through its own sexp-eval, and nudges one bounded
policy parameter toward the law's value — a real, non-overclaimed instance of the agent
rewriting a piece of itself from an equation read out of its latent dynamics."
  (if (not gate-open)
      (list :applied nil :reason :gate-closed :param current)
      (let* ((law (%identify-readout-law coefficients basis-names))
             (evaluated (%eval-law law bindings))
             (target (%ei-clamp (abs evaluated) lo hi))
             (after (%least-action-parameter current target lo hi :rate rate)))
        (list :applied t
              :law law
              :description-length (%law-description-length law)
              :solomonoff-prior (%law-solomonoff-prior law)
              :evaluated evaluated
              :target target
              :param-before current
              :param-after after
              :bounded (and (<= lo after) (<= after hi))))))
