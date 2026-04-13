;;; repl-loop.lisp — The Harmonic REPL: one path, pure functional, drives any model.
;;;
;;; The REPL is the agent's brain. It sends s-expression prompts to the LLM,
;;; evaluates the response as code, and feeds the result back. The model
;;; drives the system through primitives — recall, basin, store, exec, etc.
;;;
;;; Protocol: model outputs s-expressions. If output starts with ( → eval.
;;; If not → natural language final answer. No heuristics, no workarounds.
;;; Every round is scored. Errors downgrade the model. The agent never fails.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; SEXP DETECTION — unambiguous protocol, no heuristics
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Output starts with ( → it's code. Period. No false-positive filtering.
   The model was told to output s-expressions. If it does, we eval.
   If it doesn't, it's natural language — which is the final answer."
  (and text (stringp text) (> (length text) 1)
       (char= (char (string-trim '(#\Space #\Newline #\Return #\Tab) text) 0) #\()))

(defun %reject-reader-macros (text)
  "Signal error if TEXT contains reader macro dispatch sequences.
Only #\\ (character literal) is benign; all others are rejected."
  (loop for i from 0 below (1- (length text))
        when (and (char= (char text i) #\#)
                  (not (char= (char text (1+ i)) #\\)))
          do (error "reader macro rejected: #~A" (char text (1+ i)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; FORM EVALUATION — read in :harmonia package, eval restricted
;;; ═══════════════════════════════════════════════════════════════════════

(defun %eval-all-forms (text)
  "Parse text as restricted Lisp forms and evaluate each. Return combined results."
  (%reject-reader-macros text)
  (let ((*read-eval* nil)
        (*package* (find-package :harmonia))  ;; symbols in our package for case dispatch
        (results '())
        (env '()))
    (handler-case
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
;;; MODEL PERFORMANCE — the REPL rates models by how they use it
;;; ═══════════════════════════════════════════════════════════════════════

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
      (setf (gethash model *repl-model-perf*) perf)
      ;; Pipeline trace: model performance update
      (%pipeline-trace :model-perf-update
        :model model :outcome outcome :latency-ms latency-ms
        :fluency (%repl-fluency model)
        :speed (%repl-speed model)
        :score (%repl-model-score model)
        :code-ok (or (getf perf :code-ok) 0)
        :code-error (or (getf perf :code-error) 0)
        :natural (or (getf perf :natural) 0)
        :unavailable (or (getf perf :unavailable) 0)))))

(defun %repl-fluency (model)
  "How well does the model speak s-expressions? [0.0-1.0]
   fluency = code-ok / (code-ok + code-error + unavailable)"
  (let* ((perf (gethash model *repl-model-perf*))
         (ok (or (getf perf :code-ok) 0))
         (err (or (getf perf :code-error) 0))
         (recall (or (getf perf :recall) 0))
         (fail (+ (or (getf perf :error) 0) (or (getf perf :unavailable) 0)))
         (total (+ ok err recall fail)))
    (if (< total 3) 0.5 (/ (float (+ ok recall)) (float total)))))

(defun %repl-speed (model)
  "Average latency score [0.0-1.0]. Sigmoid: 1000ms→0.73, 3000ms→0.5"
  (let* ((perf (gethash model *repl-model-perf*))
         (total-ms (or (getf perf :total-ms) 0))
         (calls (max 1 (or (getf perf :calls) 1)))
         (avg-ms (/ total-ms calls)))
    (/ 1.0 (+ 1.0 (exp (/ (- avg-ms 3000.0) 2000.0))))))

(defun %repl-model-score (model)
  "Combined REPL score: 0.5×fluency + 0.3×speed + 0.2×(1-cost)."
  (let* ((fluency (%repl-fluency model))
         (speed (%repl-speed model))
         (profile (handler-case (%profile-by-id model) (error () nil)))
         (cost (if profile (or (getf profile :cost) 5) 5))
         (cost-factor (/ 1.0 (+ 1.0 (float cost)))))
    (+ (* 0.5 fluency) (* 0.3 speed) (* 0.2 cost-factor))))

(defun %select-model-by-repl-perf (prompt)
  "Select best model by measured REPL performance. Purely data-driven."
  (declare (ignore prompt))
  (let* ((tier-pool (handler-case (%tier-model-pool *routing-tier*) (error () nil)))
         (all-pool (or tier-pool
                       (handler-case (%tier-model-pool :auto) (error () nil))
                       '()))
         (scored (mapcar (lambda (m) (cons m (%repl-model-score m))) all-pool))
         (ranked (sort scored #'> :key #'cdr))
         (chosen (or (car (first ranked)) "")))
    (%pipeline-trace :model-ranking
      :tier *routing-tier* :pool-size (length all-pool) :chosen chosen
      :top-3 (format nil "~{~A=~,3F~^ | ~}"
               (loop for (m . s) in (subseq ranked 0 (min 3 (length ranked)))
                     collect m collect s)))
    chosen))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE HARMONIC REPL — minimal, pure functional, drives any model
;;; ═══════════════════════════════════════════════════════════════════════

(defun %repl-boot-prompt (agent-name user-text)
  "L0 boot: teach the model HOW to think, not WHAT to know.
   Step 1: call (field) to get global context map
   Step 2: follow the chain — field→recall→status→respond
   The field guides the model through memory layers automatically."
  (concatenate 'string
    (format nil ";; ~A REPL. Output ONE s-expression per turn.~%" agent-name)
    ";; Start with (field) to see your global context and capabilities.
;; Then: (recall \"q\") for user memories, (status) for system state.
;; Tools: (exec \"cmd\") (read-file \"p\") (grep \"p\" \"d\") (fetch \"url\")
;; (python \"code\") (search \"q\") (store \"text\") (datamine \"lode\")
;; (respond \"answer\") when ready. (str a b) joins. (let ((x (f))) (respond x)) chains.
"
    (format nil ";; user: ~A" user-text)))

(defun %repl-continuation-prompt (round agent-name last-result user-text)
  "Continuation: show result + remind primitives. Models forget between rounds."
  (format nil ";; ~A R~D. Result:~%~A~%;; Available: (respond \"ans\") (recall \"q\") (exec \"cmd\") (field) (basin) (store \"t\") (python \"code\") (fetch \"url\") (str a b)~%;; user: ~A"
          agent-name round
          (%clip-prompt (or last-result "(nil)") 600)
          user-text))

(defun %clip-prompt (text &optional (limit 256))
  (let ((s (or text "")))
    (if (<= (length s) limit) s (subseq s 0 limit))))

(defun %orchestrate-repl (prompt &key (max-rounds *repl-max-rounds*))
  "ONE path. Boot prompt (L0) → send → eval → loop. Pure functional.
   No memory injected into prompt — model discovers via REPL primitives.
   L1 field = global context. L2 chronicle = system log. L3 palace = user data."
  (let* ((user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-payload prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         (current-prompt (%repl-boot-prompt (%agent-name) user-text))
         (round 0)
         (last-eval-result nil))

    (%log :info "sexp-eval" "REPL: len=~D user=[~A]"
          (length current-prompt)
          (subseq user-text 0 (min 60 (length user-text))))
    (%pipeline-trace :repl-enter
      :prompt-len (length current-prompt)
      :user-text-len (length user-text)
      :memory-recalled 0
      :max-rounds max-rounds
      :routing-tier *routing-tier*)

    ;; The (respond ...) primitive throws 'repl-respond to exit the loop.
    ;; Both paths (throw and return-from) go through %repl-auto-store-and-return.
    (%repl-auto-store-and-return user-text
    (catch 'repl-respond
      (loop while (< round max-rounds) do
        (incf round)
        (let ((round-prompt
                (if (= round 1)
                    current-prompt
                    (%repl-continuation-prompt round (%agent-name) last-eval-result user-text)))
              (used-model (or (handler-case (%select-model user-text) (error () nil)) ""))
              (call-start (get-internal-real-time)))

          ;; Trace prompt sent to LLM
          (%pipeline-trace :repl-llm-prompt
            :round round :model used-model
            :prompt-len (length round-prompt)
            :prompt-content (%clip-prompt round-prompt 800))

          (let ((llm-output
                  (handler-case (backend-complete round-prompt used-model)
                    (error (c)
                      (%log :warn "sexp-eval" "REPL ~D error: ~A" round c)
                      (%record-repl-perf used-model :error)
                      nil)))
                (latency-ms (truncate (* 1000 (/ (- (get-internal-real-time) call-start)
                                                  (float internal-time-units-per-second))))))
            (cond
              ;; No response — model unavailable
              ((null llm-output)
               (%log :info "sexp-eval" "REPL ~D: LLM unavailable" round)
               (%pipeline-trace :repl-round :round round :model used-model
                 :response-type "unavailable" :response-len 0)
               (%record-repl-perf used-model :unavailable :latency-ms latency-ms)
               (when last-eval-result
                 (return-from %orchestrate-repl
                   (format nil "Based on what I found: ~A"
                           (subseq last-eval-result 0
                                   (min 800 (length last-eval-result))))))
               (return-from %orchestrate-repl nil))

              ;; Output starts with ( → code. Evaluate.
              ((%is-sexp-output-p llm-output)
               (%log :info "sexp-eval" "REPL ~D: evaluating code" round)
               (%pipeline-trace :repl-sexp-generated
                 :round round :model used-model
                 :sexp-content (%clip-prompt llm-output 500)
                 :latency-ms latency-ms)
               (%pipeline-trace :repl-round :round round :model used-model
                 :response-type "sexp-code" :response-len (length llm-output))
               (let ((eval-result (handler-case (%eval-all-forms llm-output)
                                    (error (e)
                                      (%log :warn "sexp-eval" "REPL ~D: eval failed: ~A" round e)
                                      nil))))
                 (if (and eval-result (> (length eval-result) 0)
                          (not (search "parse-error" eval-result)))
                     (progn
                       (setf last-eval-result eval-result)
                       (%pipeline-trace :repl-sexp-eval-ok
                         :round round :model used-model
                         :eval-result (%clip-prompt eval-result 300))
                       (%record-repl-perf used-model :code-ok :latency-ms latency-ms))
                     (progn
                       (%pipeline-trace :repl-sexp-eval-fail
                         :round round :model used-model
                         :sexp-attempted (%clip-prompt llm-output 200))
                       (%record-repl-perf used-model :code-error :latency-ms latency-ms)
                       ;; Error feeds back into the loop — model sees what failed
                       (setf last-eval-result
                             (or eval-result (format nil "(:eval-error \"~A\")" (%clip-prompt llm-output 100))))))))

              ;; Natural language → final answer from model
              (t
               (%record-repl-perf used-model :natural :latency-ms latency-ms)
               (%log :info "sexp-eval" "REPL ~D: response (~D chars)" round (length llm-output))
               (%pipeline-trace :repl-round :round round :model used-model
                 :response-type "natural-language" :response-len (length llm-output))
               (%pipeline-trace :response-delivery
                 :frontend (if (harmonia-signal-p prompt) (harmonia-signal-frontend prompt) "tui")
                 :response-len (length llm-output) :model used-model :latency-ms latency-ms)
               (return-from %orchestrate-repl
                 (%repl-auto-store-and-return user-text llm-output)))))))))))

(defun %repl-auto-store-and-return (user-text response)
  "Store interaction as memory, then return the response."
  (when (and response (stringp response) (> (length response) 30))
    (handler-case
        (progn
          (memory-put :interaction
                      (format nil "Q: ~A~%A: ~A" user-text (%clip-prompt response 500))
                      :tags '(:repl :interaction))
          (%pipeline-trace :memory-auto-store
            :query-len (length user-text)
            :response-len (length response)))
      (error () nil)))
  response)
