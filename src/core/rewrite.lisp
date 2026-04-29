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
  "Execute evolution when vitruvian gate opens. Full pipeline:
   gate check → prepare → record to memory → execute → record to ouroboros."
  (when (or (search "evolve" (string-downcase prompt))
            (%harmonic-plan-ready-p))
    (incf (runtime-state-rewrite-count *runtime*))
    ;; Record evolution trigger to memory (L1 field + L3 palace)
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
    ;; Execute evolution pipeline: prepare → execute → record
    (handler-case
        (let ((prep (when (fboundp 'evolution-prepare) (evolution-prepare))))
          (when (and prep (eq (getf prep :health) :ready))
            (let ((result (when (fboundp 'evolution-execute)
                            (evolution-execute
                              :component "repl"
                              :patch-body (format nil "evolution-trigger cycle=~D"
                                                  (runtime-state-cycle *runtime*))))))
              ;; Record to ouroboros crash ledger for audit trail
              (when (fboundp 'ouroboros-write-patch)
                (ouroboros-write-patch "evolution"
                  (format nil "(:evolution :cycle ~D :count ~D :snapshot ~A)"
                          (runtime-state-cycle *runtime*)
                          (runtime-state-rewrite-count *runtime*)
                          (or (getf result :snapshot) "nil"))))
              (%log :info "evolution" "Evolution executed: ~A" result))))
      (error (e) (%log :warn "evolution" "Evolution failed: ~A" e)))
    t))
