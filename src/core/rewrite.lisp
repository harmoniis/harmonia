;;; rewrite.lisp — Controlled self-rewrite hooks.

(in-package :harmonia)

(defun %harmonic-plan-ready-p ()
  (let* ((ctx (and *runtime* (runtime-state-harmonic-context *runtime*)))
         (plan (and ctx (getf ctx :plan)))
         (vitruvian (and plan (getf plan :vitruvian))))
    (and plan
         (getf plan :ready)
         vitruvian
         (>= (getf vitruvian :signal 0.0)
             (harmony-policy-number "rewrite-plan/signal-min" 0.62))
         (<= (getf vitruvian :noise 1.0)
             (harmony-policy-number "rewrite-plan/noise-max" 0.38)))))

(defun %source-rewrite-enabled-p ()
  (let ((raw (config-get-for "evolution" "source-rewrite-enabled")))
    (if raw
        (member (string-downcase raw) '("1" "true" "yes" "on") :test #'string=)
        t)))

(defun maybe-self-rewrite (prompt response)
  "Trigger minimal rewrite bookkeeping when explicitly requested.
   Real code mutation engine can replace this implementation later."
  (unless (%source-rewrite-enabled-p)
    (return-from maybe-self-rewrite nil))
  (when (or (search "rewrite" (string-downcase prompt))
            (%harmonic-plan-ready-p))
    (incf (runtime-state-rewrite-count *runtime*))
    (memory-put :skill
                (list :rewrite-trigger :harmonic-plan
                      :prompt prompt
                      :response-preview (subseq response 0 (min 120 (length response)))
                      :cycle (runtime-state-cycle *runtime*))
                :depth 1
                :tags (list :rewrite :harmony :vitruvian))
    (runtime-log *runtime* :rewrite-triggered
                 (list :count (runtime-state-rewrite-count *runtime*)
                       :hint (subseq response 0 (min 80 (length response)))))
    t))
