;;; rewrite.lisp — Runtime evolution hooks.
;;; The vitruvian gate reads thresholds from DNA constraints.
;;; Evolution = binary rollout + REPL runtime adaptation. No source code rewrite.

(in-package :harmonia)

(defun %harmonic-plan-ready-p ()
  "Check if the harmonic machine says we're ready to evolve.
   Thresholds come from DNA constraints — these are the hard barriers."
  (let* ((ctx (and *runtime* (runtime-state-harmonic-context *runtime*)))
         (plan (and ctx (getf ctx :plan)))
         (vitruvian (and plan (getf plan :vitruvian)))
         (signal-min (or (handler-case (dna-constraint :rewrite-signal-min) (error () nil)) 0.62))
         (noise-max (or (handler-case (dna-constraint :rewrite-noise-max) (error () nil)) 0.38)))
    (and plan
         (getf plan :ready)
         vitruvian
         (>= (getf vitruvian :signal 0.0)
             (signalograd-effective-harmony-number "rewrite-plan/signal-min" signal-min *runtime*))
         (<= (getf vitruvian :noise 1.0)
             (harmony-policy-number "rewrite-plan/noise-max" noise-max)))))

(defun maybe-self-rewrite (prompt response)
  "Record evolution event when vitruvian gate opens. Binary rollout, not source rewrite."
  (when (or (search "evolve" (string-downcase prompt))
            (%harmonic-plan-ready-p))
    (incf (runtime-state-rewrite-count *runtime*))
    (memory-put :skill
                (list :rewrite-trigger :harmonic-plan
                      :prompt prompt
                      :response-preview (subseq response 0 (min 120 (length response)))
                      :cycle (runtime-state-cycle *runtime*))
                :depth 1
                :tags (list :evolution :harmony :vitruvian))
    (runtime-log *runtime* :rewrite-triggered
                 (list :count (runtime-state-rewrite-count *runtime*)
                       :hint (subseq response 0 (min 80 (length response)))))
    t))
