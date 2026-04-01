;;; rewrite.lisp — Controlled self-rewrite hooks.
;;; The vitruvian gate reads thresholds from DNA constraints.
;;; Evolution requires DNA permission. Epigenetic tuning is softer.

(in-package :harmonia)

(defun %harmonic-plan-ready-p ()
  "Check if the harmonic machine says we're ready to evolve.
   Thresholds come from DNA constraints — these are the hard barriers."
  (let* ((ctx (and *runtime* (runtime-state-harmonic-context *runtime*)))
         (plan (and ctx (getf ctx :plan)))
         (vitruvian (and plan (getf plan :vitruvian)))
         ;; Read gates from DNA. These are the biological barriers.
         (signal-min (or (ignore-errors (dna-constraint :rewrite-signal-min)) 0.62))
         (noise-max (or (ignore-errors (dna-constraint :rewrite-noise-max)) 0.38)))
    (and plan
         (getf plan :ready)
         vitruvian
         (>= (getf vitruvian :signal 0.0)
             (signalograd-effective-harmony-number "rewrite-plan/signal-min" signal-min *runtime*))
         (<= (getf vitruvian :noise 1.0)
             (harmony-policy-number "rewrite-plan/noise-max" noise-max)))))

(defun %source-rewrite-enabled-p ()
  (let ((raw (config-get-for "evolution" "source-rewrite-enabled")))
    (if raw
        (member (string-downcase raw) '("1" "true" "yes" "on") :test #'string=)
        t)))

(defun maybe-self-rewrite (prompt response)
  "Trigger evolution when vitruvian gate opens.
   Records the event and calls Ouroboros if wired."
  (unless (%source-rewrite-enabled-p)
    (return-from maybe-self-rewrite nil))
  (when (or (search "rewrite" (string-downcase prompt))
            (%harmonic-plan-ready-p))
    (incf (runtime-state-rewrite-count *runtime*))
    ;; Record evolution event in memory field.
    (memory-put :skill
                (list :rewrite-trigger :harmonic-plan
                      :prompt prompt
                      :response-preview (subseq response 0 (min 120 (length response)))
                      :cycle (runtime-state-cycle *runtime*))
                :depth 1
                :tags (list :rewrite :harmony :vitruvian))
    ;; Log to Ouroboros crash ledger (evolution events are lifecycle events).
    (ignore-errors
      (when (fboundp 'ouroboros-record-crash)
        (funcall 'ouroboros-record-crash "evolution"
                 (format nil "rewrite-triggered cycle=~D"
                         (runtime-state-cycle *runtime*)))))
    (runtime-log *runtime* :rewrite-triggered
                 (list :count (runtime-state-rewrite-count *runtime*)
                       :hint (subseq response 0 (min 80 (length response)))))
    t))
