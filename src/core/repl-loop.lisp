;;; repl-loop.lisp — REPL orchestration loop, parse/detect, model performance tracking.
;;; Cascade pattern: try models in ranked order, circuit-break failures instantly.

(in-package :harmonia)

;;; ─── Model Cascade (Handle/Service Pattern) ──────────────────────
;;; Try primary model, on failure cascade through selection chain.
;;; Each failure: circuit breaker records, callback notified, next model tried.
;;; Pure functional: cascade is a fold over the model chain.

(defun %try-models-cascade (prompt primary-model user-text on-failure-fn)
  "Try PRIMARY-MODEL first, then cascade through selection chain on failure.
   ON-FAILURE-FN is called with (model error-string) for each failure.
   Returns the first successful response, or nil if all fail."
  (flet ((try-model (model)
           (handler-case (backend-complete prompt model)
             (error (c)
               (funcall on-failure-fn model (princ-to-string c))
               nil))))
    ;; Try primary first.
    (let ((result (try-model primary-model)))
      (when result (return-from %try-models-cascade result)))
    ;; Cascade through ranked alternatives.
    (let ((chain (handler-case (%selection-chain-tiered user-text) (error () nil))))
      (dolist (alt-model (or chain '()))
        (unless (string= alt-model primary-model)
          (let ((result (try-model alt-model)))
            (when result (return-from %try-models-cascade result))))))
    nil))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PARALLEL CONTEXT GATHERING
;;; ═══════════════════════════════════════════════════════════════════════

;; Parallel gather removed — memory-recall is the ONE recall path.

;;; ═══════════════════════════════════════════════════════════════════════
;;; PARSE & DETECT
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Does the LLM output contain s-expressions to evaluate?"
  (when (and text (stringp text) (> (length text) 2))
    (let ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text)))
      (and (> (length trimmed) 2)
           (char= (char trimmed 0) #\()
           ;; Not a false positive: (I think...) is not code.
           (not (and (> (length trimmed) 3)
                     (alpha-char-p (char trimmed 1))
                     (not (position #\Space (subseq trimmed 1 (min 15 (length trimmed)))))))))))

(defun %reject-reader-macros (text)
  "Signal error if TEXT contains reader macro dispatch sequences.
Only #\\ (character literal) is benign; all others are rejected."
  (loop for i from 0 below (1- (length text))
        when (char= (char text i) #\#)
        do (let ((next (char text (1+ i))))
             (unless (char= next #\\)  ; #\ is safe (character literal)
               (error "Rejected reader macro #~C at position ~D" next i)))))

(defun %eval-all-forms (text)
  "Parse text as restricted Lisp forms and evaluate each. Return combined results."
  (%reject-reader-macros text)  ;; Block reader macros before any read
  (let ((*read-eval* nil)
        (results '())
        (env '()))  ;; empty lexical environment
    (handler-case
        ;; Try to read all forms from the text.
        (with-input-from-string (stream text)
          (loop for form = (handler-case (read stream nil :eof)
                             (error () :eof))
                until (eq form :eof)
                do (let ((result (handler-case
                                     (%reval form env)
                                   (error (c)
                                     (format nil "(:error \"~A\")" (princ-to-string c))))))
                     (%log :info "sexp-eval" "Eval: ~A → ~D chars"
                           (subseq (princ-to-string form) 0
                                   (min 60 (length (princ-to-string form))))
                           (length (princ-to-string result)))
                     (push (princ-to-string result) results))))
      (error (c)
        (push (format nil "(:parse-error \"~A\")" (princ-to-string c)) results)))
    (format nil "~{~A~%~}" (nreverse results))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; MODEL PERFORMANCE TRACKING — the REPL rates models by how they use it
;;; ═══════════════════════════════════════════════════════════════════════

;; Per-model REPL performance: fluency (valid code), speed, intelligence.
;; Models that speak s-expressions correctly score higher.
;; No hardcoded model preferences — measured performance decides.

(defparameter *repl-model-perf* (make-hash-table :test 'equal)
  "model-id → (:code-ok N :code-error N :natural N :recall N :error N
               :unavailable N :total-ms N :calls N)")

(defun %record-repl-perf (model outcome &key (latency-ms 0))
  "Record one REPL interaction outcome for a model."
  (when (and model (stringp model) (> (length model) 0))
    (let ((perf (or (gethash model *repl-model-perf*) '())))
      (setf (getf perf outcome) (1+ (or (getf perf outcome) 0)))
      (when (> latency-ms 0)
        (setf (getf perf :total-ms) (+ (or (getf perf :total-ms) 0) latency-ms))
        (setf (getf perf :calls) (1+ (or (getf perf :calls) 0))))
      (setf (gethash model *repl-model-perf*) perf))))

(defun %repl-fluency (model)
  "REPL fluency score [0.0-1.0]. How well does the model speak s-expressions?
   fluency = (code-ok + recall) / (code-ok + code-error + recall + error + unavailable)
   Models that never tried code get 0.5 (unknown)."
  (let* ((perf (gethash model *repl-model-perf*))
         (ok (or (getf perf :code-ok) 0))
         (err (or (getf perf :code-error) 0))
         (recall (or (getf perf :recall) 0))
         (fail (+ (or (getf perf :error) 0) (or (getf perf :unavailable) 0)))
         (total (+ ok err recall fail)))
    (if (< total 3) 0.5  ;; Not enough data — neutral.
        (/ (float (+ ok recall)) (float total)))))

(defun %repl-speed (model)
  "Average latency score [0.0-1.0]. 1.0 = instant, 0.0 = very slow."
  (let* ((perf (gethash model *repl-model-perf*))
         (total-ms (or (getf perf :total-ms) 0))
         (calls (max 1 (or (getf perf :calls) 1)))
         (avg-ms (/ total-ms calls)))
    ;; Sigmoid: 1000ms → 0.73, 3000ms → 0.5, 10000ms → 0.17
    (/ 1.0 (+ 1.0 (exp (/ (- avg-ms 3000.0) 2000.0))))))

(defun %repl-model-score (model)
  "Combined REPL score: 0.5×fluency + 0.3×speed + 0.2×(1-cost).
   Higher = better. Models with no data get 0.5 (try them)."
  (let* ((fluency (%repl-fluency model))
         (speed (%repl-speed model))
         (profile (handler-case (%profile-by-id model) (error () nil)))
         (cost (if profile (or (getf profile :cost) 5) 5))
         (cost-factor (/ 1.0 (+ 1.0 (float cost)))))
    (+ (* 0.5 fluency) (* 0.3 speed) (* 0.2 cost-factor))))

(defun %select-model-by-repl-perf (prompt)
  "Select the best model by measured REPL performance. Start from free, escalate.
   No hardcoded model names — purely data-driven."
  (let* ((tier-pool (handler-case (%tier-model-pool *routing-tier*) (error () nil)))
         (all-pool (or tier-pool
                       (handler-case (%tier-model-pool :auto) (error () nil))
                       '()))
         ;; Score each model by REPL performance.
         (scored (mapcar (lambda (m) (cons m (%repl-model-score m))) all-pool))
         ;; Sort: highest score first.
         (ranked (sort scored #'> :key #'cdr)))
    (or (car (first ranked)) "")))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE HARMONIC REPL — the orchestration core
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *repl-max-rounds*))
  "ONE path. Recall context from memory. Send to LLM. Eval response.
No score branching, no model selection, no bootstrap modes. ONE path."
  (let* ((user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-payload prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         ;; Recall from memory field — s-expression context.
         (recalled (handler-case
     (let ((entries (memory-recall user-text :limit 5)
   (error () nil)))
                       (when entries
                         (with-output-to-string (out)
                           (dolist (e entries)
                             (let ((text (%entry-text e)))
                               (when (and (stringp text) (> (length text) 10))
                                 (write-string (subseq text 0 (min 200 (length text))) out)
                                 (terpri out)))))))))
         ;; Structural frame with Lisp comments as bridge markers.
         ;; Comments are structural in Lisp — they're part of the language.
         ;; The LLM sees the structure AND can parse the markers.
         (current-prompt
           (format nil ";; agent: ~A~%;; output: s-expression → eval | natural-language → user~%;; (respond \"text\") to answer. (recall q) (read-file p) (grep p) (exec c) (status) (list-files d)~%~A~%~%;; query:~%~A"
                   (%agent-name)
                   (if (and recalled (> (length recalled) 0))
                       (format nil "~%;; context:~%~A" recalled)
                       "")
                   user-text))
         (round 0)
         (last-eval-result nil))

    (%log :info "sexp-eval" "REPL: len=~D user=[~A]"
          (length current-prompt)
          (subseq user-text 0 (min 60 (length user-text))))

    ;; The (respond ...) primitive throws 'repl-respond to exit the loop.
    (catch 'repl-respond
      (loop while (< round max-rounds) do
        (incf round)
        (let ((round-prompt
                (if (= round 1)
                    current-prompt
                    (format nil ";; agent: ~A~%;; result:~%~A~%~%;; query:~%~A"
                            (%agent-name)
                            (or last-eval-result "")
                            user-text)))
              (used-model (or (handler-case (%select-model user-text) (error () nil)) ""))
              (call-start (get-internal-real-time)))

          ;; CASCADE with structured error feedback.
          ;; Errors flow into: circuit breaker, signalograd, memory field, and REPL.
          (let* ((failed-models '())
                 (llm-output
                   (%try-models-cascade
                    round-prompt used-model user-text
                    (lambda (model err)
                      (push (list model err) failed-models)
                      (%log :warn "sexp-eval" "REPL ~D: ~A failed: ~A" round model err)
                      (model-policy-record-outcome :model model :success nil :latency-ms 0))))
                 (latency-ms (truncate (* 1000 (/ (- (get-internal-real-time) call-start)
                                                   (float internal-time-units-per-second))))))
            ;; Record errors in memory field — system learns failure patterns.
            (when failed-models
              (handler-case
                  (memory-put :tool
                    (format nil "(:provider-errors ~{(:model ~S :error ~S)~^ ~})"
                            (reduce #'append failed-models))
                    :depth 0 :tags '(:provider-error :system-health))
                (error () nil)))
            (cond
              ((null llm-output)
               (%log :info "sexp-eval" "REPL ~D: all models unavailable" round)
               (%record-repl-perf used-model :unavailable :latency-ms latency-ms)
               ;; Feed error context to REPL — LLM understands what happened.
               (when last-eval-result
                 (return-from %orchestrate-repl
                   (format nil "Based on what I found: ~A"
                           (subseq last-eval-result 0
                                   (min 800 (length last-eval-result))))))
               (return-from %orchestrate-repl nil))

              ;; Output starts with ( → it's code. Evaluate via restricted interpreter.
              ((%is-sexp-output-p llm-output)
               (%log :info "sexp-eval" "REPL ~D: evaluating code" round)
               (let ((eval-result (handler-case (%eval-all-forms llm-output)
                                    (error (e)
                                      (%log :warn "sexp-eval" "REPL ~D: eval failed: ~A" round e)
                                      nil))))
                 (if (and eval-result (> (length eval-result) 0)
                          (not (search "parse-error" eval-result)))
                     (progn
                       (setf last-eval-result eval-result)
                       (%record-repl-perf used-model :code-ok :latency-ms latency-ms))
                     ;; Eval failed — strip code markers and return as natural language.
                     (progn
                       (%record-repl-perf used-model :code-error :latency-ms latency-ms)
                       (let ((cleaned (string-trim '(#\( #\) #\Space #\Newline) llm-output)))
                         (when (> (length cleaned) 10)
                           (return-from %orchestrate-repl cleaned)))))))

              ;; RECALL: keyword → the simple model wants more context.
              ((search "RECALL:" llm-output)
               (%record-repl-perf used-model :recall :latency-ms latency-ms)
               (let* ((pos (search "RECALL:" llm-output))
                      (query (string-trim '(#\Space #\Newline #\Return)
                                          (subseq llm-output (+ pos 7))))
                      (more (handler-case
     (memory-semantic-recall-block query :limit 3 :max-chars 800)
   (error () nil))))
                 (%log :info "sexp-eval" "REPL ~D: RECALL query=[~A]" round
                       (subseq query 0 (min 40 (length query))))
                 (setf last-eval-result (or more "(no additional context found)"))))

              ;; Natural language → return to user. Meditate on success.
              (t
               (%record-repl-perf used-model :natural :latency-ms latency-ms)
               (%log :info "sexp-eval" "REPL ~D: response (~D chars)" round (length llm-output))
               (return-from %orchestrate-repl llm-output))))))

      ;; Exceeded rounds — structural summary request.
      (when last-eval-result
        (%log :info "sexp-eval" "REPL: final summary from eval data")
        (handler-case
            (backend-complete
             (format nil ";; agent: ~A~%;; data:~%~A~%~%;; query:~%~A"
                     (%agent-name)
                     (subseq last-eval-result 0 (min 1200 (length last-eval-result)))
                     user-text)
             (or (handler-case (%select-model user-text) (error () nil)) ""))
          (error () nil))))))
