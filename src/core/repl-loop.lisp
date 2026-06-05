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
  "True when TEXT starts with a REPL form whose operator is known.
The boundary is structural: registered primitives plus evaluator special forms."
  (when (and text (stringp text))
    (let ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text)))
      (and (> (length trimmed) 1)
           (char= (char trimmed 0) #\()
           (handler-case
               (progn
                 (%reject-reader-macros trimmed)
                 (let ((*read-eval* nil)
                       (*package* (find-package :harmonia)))
                   (with-input-from-string (stream trimmed)
                     (let ((form (read stream nil :eof)))
                       (and (consp form)
                            (%repl-known-operator-p (car form)))))))
             (error () nil))))))

(defun %reject-reader-macros (text)
  "Signal error if TEXT contains reader macro dispatch sequences.
Only #\\ (character literal) is benign; all others are rejected."
  (loop for i from 0 below (1- (length text))
        when (and (char= (char text i) #\#)
                  (not (char= (char text (1+ i)) #\\)))
          do (error "reader macro rejected: #~A" (char text (1+ i)))))

(defun %repl-read-forms (text)
  "Read all model forms as data in the restricted REPL package."
  (%reject-reader-macros text)
  (let ((*read-eval* nil)
        (*package* (find-package :harmonia)))
    (with-input-from-string (stream text)
      (loop for form = (read stream nil :eof)
            until (eq form :eof)
            collect form))))

(defun %repl-code-signature (text)
  "Canonical structural signature used to detect a stalled REPL loop."
  (with-output-to-string (out)
    (dolist (form (%repl-read-forms text))
      (write form :stream out :readably t)
      (terpri out))))

(defparameter *repl-observation-operators*
  '(field status basin env introspect models chaos-risk)
  "Read-only context calls whose raw result is not a user-facing answer.")

(defun %repl-observation-only-p (text)
  "True when every model form only inspects context."
  (let ((forms (handler-case (%repl-read-forms text) (error () nil))))
    (and forms
         (every (lambda (form)
                  (and (consp form)
                       (member (car form) *repl-observation-operators* :test #'eq)))
                forms))))

(defun %repl-explicit-observation-request-p (user-text code)
  "True when USER-TEXT names an observation primitive called by CODE."
  (let ((words (%split-words user-text))
        (forms (handler-case (%repl-read-forms code) (error () nil))))
    (some (lambda (form)
            (and (consp form)
                 (member (car form) *repl-observation-operators* :test #'eq)
                 (member (string-downcase (symbol-name (car form)))
                         words
                         :test #'string=)))
          forms)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; FORM EVALUATION — read in :harmonia package, eval restricted
;;; ═══════════════════════════════════════════════════════════════════════

(defun %eval-all-forms (text)
  "Parse TEXT as restricted Lisp forms, evaluate each.
   Returns (values OUTPUT ERRORED-P). ERRORED-P is true if any form raised a
   condition or any result is an error s-expression — so the caller can score
   the model honestly instead of treating an (:error ...) value as success."
  (%reject-reader-macros text)
  (let ((*read-eval* nil)
        (*package* (find-package :harmonia))  ;; symbols in our package for case dispatch
        (results '())
        (errored nil)
        (env '()))
    (handler-case
        (with-input-from-string (stream text)
          (loop for form = (handler-case (read stream nil :eof)
                             (error () :eof))
                until (eq form :eof)
                do (let ((result (handler-case
                                     (%reval form env)
                                   (error (c)
                                     (setf errored t)
                                     (format nil "(:error \"~A\")" (princ-to-string c))))))
                     ;; Bound at DISPLAY time only — raw values flowed through let bindings intact.
                     (let ((printed (%bound-result (princ-to-string result))))
                       (when (%error-form-p printed) (setf errored t))
                       (push printed results)))))
      (error (c)
        (setf errored t)
        (push (format nil "(:parse-error \"~A\")" (princ-to-string c)) results)))
    (values (format nil "~{~A~%~}" (nreverse results)) errored)))

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
  ;; Honor the persisted tier (same source as choose-model) so both selection paths
  ;; agree — otherwise /premium would gate one path but not the other.
  (when (fboundp '%load-routing-tier) (%load-routing-tier))
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

(defparameter *repl-frame-examples*
  '((field      . "(field)")
    (recall     . "(recall \"topic\")")
    (status     . "(status)")
    (basin      . "(basin)")
    (store      . "(store \"text to remember\")")
    (exec       . "(exec \"uname -a\")")
    (fetch      . "(fetch \"https://example.com\")")
    (search     . "(search \"query\")")
    (read-file  . "(read-file \"/path/to/file\")")
    (grep       . "(grep \"pattern\" \"/path\")")
    (python     . "(python \"print(2+2)\")"))
  "Concrete call examples — the teaching surface for the restricted dialect.
   Dumb models copy what they see verbatim, so we show exact calls with real
   arguments, never lambda-list keywords. Rendered only for primitives that
   actually exist in *primitive-dispatch* (computed, not asserted).")

(defparameter *repl-frame-answer-examples*
  '((field  . "(respond (field))")
    (status . "(respond (status))")
    (recall . "(respond (recall \"topic\"))")
    (store  . "(respond (store \"text to remember\"))")
    (exec   . "(respond (exec \"uname -a\"))")
    (python . "(respond (python \"print(2+2)\"))"))
  "Complete one-expression action-to-answer forms for weak models.")

(defun %compute-repl-frame ()
  "REPL instruction frame — concrete call examples derived from the dispatch table.
   No lambda-lists (&key/&optional/&rest): models echo them literally and fail."
  (let ((lines '())
        (answers '()))
    (dolist (pair *repl-frame-examples*)
      (when (gethash (car pair) *primitive-dispatch*)
        (push (format nil ";;   ~A" (cdr pair)) lines)))
    (dolist (pair *repl-frame-answer-examples*)
      (when (and (%repl-known-operator-p 'respond)
                 (gethash (car pair) *primitive-dispatch*))
        (push (format nil ";;   ~A" (cdr pair)) answers)))
    (format nil
            ";; Restricted Lisp REPL. Reply with ONE s-expression, nothing else.~%;; Choose the call that directly advances the user request. Never repeat a completed call.~%;; Use EXACTLY these calls, with real arguments (no &key, no &optional):~%~{~A~%~};; Complete action-to-answer forms:~%~{~A~%~}"
            (nreverse lines)
            (nreverse answers))))

;; Computed at boot from *primitive-dispatch*. Identical every round — no model confusion.
(defvar *repl-frame* nil "REPL instruction frame — computed from dispatch table.")

(defun %build-repl-prompt (agent-name user-text &key round last-result)
  "Build prompt as s-expression structure. Rendered to string at LLM boundary.
   Homoiconic: the epigenetic system can structurally modify prompts."
  (unless *repl-frame* (setf *repl-frame* (%compute-repl-frame)))
  (if (or (null round) (= round 1))
      `(:prompt
        (:system ,(format nil "~A REPL. Complete the user request directly." agent-name))
        (:frame ,*repl-frame*)
        (:user ,user-text))
      `(:prompt
        (:system ,(format nil "~A REPL. Use the previous result, choose a different call, or respond." agent-name))
        (:context ,(%clip-prompt (or last-result "") 2000))
        (:frame ,*repl-frame*)
        (:user ,user-text))))

(defun %render-prompt (prompt-sexp)
  "Render s-expression prompt to string at LLM API boundary."
  (with-output-to-string (out)
    (dolist (section (cdr prompt-sexp))
      (let ((kind (car section))
            (content (cadr section)))
        (case kind
          (:system  (format out ";; ~A~%" content))
          (:frame   (write-string content out))
          (:context (format out ";; ~A~%" content))
          (:user    (format out ";; user: ~A" content)))))))

(defun %repl-boot-prompt (agent-name user-text)
  "L0 boot: REPL frame + user query."
  (%render-prompt (%build-repl-prompt agent-name user-text)))

(defun %repl-continuation-prompt (round agent-name last-result user-text)
  "Continuation: same REPL frame + previous eval result. No 'R3' labels.
   Structurally identical to boot — models cannot distinguish rounds."
  (declare (ignore round))
  (%render-prompt (%build-repl-prompt agent-name user-text
                                       :round 2 :last-result last-result)))

(defun %clip-prompt (text &optional (limit 256))
  (let ((s (or text "")))
    (if (<= (length s) limit) s (subseq s 0 limit))))

(defun %repl-usable-response-p (response)
  (and (stringp response)
       (plusp (length response))
       (not (%error-form-p response))))

(defun %record-repl-completion (user-text response model model-prompt latency-ms success)
  "Record the actual model that completed one REPL task exactly once."
  (when (and (stringp model) (plusp (length model)))
    (let* ((task (handler-case (%task-kind user-text) (error () :general)))
           (task-hint (string-downcase (symbol-name task)))
           (cost-usd (handler-case
                         (model-policy-estimate-cost-usd model model-prompt response)
                       (error () 0.0)))
           (harmony-score (handler-case (harmonic-score user-text response)
                            (error () 0.0))))
      (handler-case
          (let ((*last-task-kind* task))
            (model-policy-record-outcome
             :model model :success success :latency-ms latency-ms
             :harmony-score harmony-score :cost-usd cost-usd))
        (error (e) (%log :warn "sexp-eval" "REPL model outcome failed: ~A" e)))
      (handler-case
          (chronicle-record-delegation
           :task-hint task-hint :model model :backend "repl"
           :reason "repl-completion" :escalated nil :cost-usd cost-usd
           :latency-ms latency-ms :success success
           :tokens-in (%token-estimate model-prompt)
           :tokens-out (%token-estimate response))
        (error (e) (%log :warn "sexp-eval" "REPL delegation record failed: ~A" e)))
      (%pipeline-trace :repl-completion
        :model model :task task-hint :success success :latency-ms latency-ms)
      t)))

(defun %orchestrate-repl (prompt &key (max-rounds *repl-max-rounds*))
  "ONE path. Boot prompt (L0) → send → eval → loop. Pure functional.
   No memory injected into prompt — model discovers via REPL primitives.
   L1 field = global context. L2 chronicle = system log. L3 palace = user data."
  (let* ((user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-payload prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         (current-prompt (%repl-boot-prompt (%agent-name) user-text))
         (round 0)
         (last-eval-result nil)
         (last-presentable-result nil)
         (attempted-model "")
         (attempted-prompt "")
         (completion-model "")
         (completion-prompt "")
         (completed-p nil)
         (total-latency-ms 0)
         (seen-code-signatures (make-hash-table :test 'equal)))

    (%log :info "sexp-eval" "REPL: len=~D user=[~A]"
          (length current-prompt)
          (subseq user-text 0 (min 60 (length user-text))))
    (%pipeline-trace :repl-enter
      :prompt-len (length current-prompt)
      :user-text-len (length user-text)
      :memory-recalled 0
      :max-rounds max-rounds
      :routing-tier *routing-tier*)

    ;; A single result path owns response delivery, memory, and routing feedback.
    ;; The inner catch identifies successful (respond ...) exits without dynamic state.
    (let* ((answer
             (loop while (< round max-rounds) do
               (incf round)
               (let ((round-prompt
                       (if (= round 1)
                           current-prompt
                           (%repl-continuation-prompt round (%agent-name) last-eval-result user-text)))
                     (used-model (or (handler-case (%select-model user-text) (error () nil)) ""))
                     (call-start (get-internal-real-time)))
                 (setf attempted-model used-model
                       attempted-prompt round-prompt)

                 (%pipeline-trace :repl-llm-prompt
                   :round round :model used-model
                   :prompt-len (length round-prompt)
                   :prompt-content (%clip-prompt round-prompt 800))

                 (handler-case
                     (when *runtime*
                       (let* ((proj (signalograd-current-projection *runtime*))
                              (plan (getf (runtime-state-harmonic-context *runtime*) :plan)))
                         (runtime-log *runtime* :repl-llm-call
                                      (list :round round
                                            :model used-model
                                            :prompt-len (length round-prompt)
                                            :rewrite-ready (and plan (getf plan :ready))
                                            :confidence (or (getf proj :confidence) 0.0)
                                            :lambdoma-ratio (and plan (getf plan :lambdoma-ratio))
                                            :chaos-risk (and plan (getf plan :chaos-risk))))))
                   (error () nil))

                 (let* ((llm-output
                          (handler-case (backend-complete round-prompt used-model)
                            (error (c)
                              (%log :warn "sexp-eval" "REPL ~D error: ~A" round c)
                              (%record-repl-perf used-model :error)
                              nil)))
                        (latency-ms
                          (truncate (* 1000 (/ (- (get-internal-real-time) call-start)
                                                (float internal-time-units-per-second))))))
                   (incf total-latency-ms latency-ms)
                   (cond
                     ((or (null llm-output)
                          (and (stringp llm-output) (zerop (length llm-output))))
                      (%log :info "sexp-eval" "REPL ~D: LLM unavailable" round)
                      (%pipeline-trace :repl-round :round round :model used-model
                        :response-type "unavailable" :response-len 0)
                      (%record-repl-perf used-model :unavailable :latency-ms latency-ms)
                      (return (or last-presentable-result
                                  "I could not complete that within the available steps.")))

                     ((%is-sexp-output-p llm-output)
                      (%log :info "sexp-eval" "REPL ~D: evaluating code" round)
                      (%pipeline-trace :repl-sexp-generated
                        :round round :model used-model
                        :sexp-content (%clip-prompt llm-output 500)
                        :latency-ms latency-ms)
                      (%pipeline-trace :repl-round :round round :model used-model
                        :response-type "sexp-code" :response-len (length llm-output))
                      (let ((normal-eval-p nil)
                            (eval-result nil)
                            (errored nil))
                        (let ((responded
                                (catch 'repl-respond
                                  (multiple-value-bind (result result-errored)
                                      (let ((signature
                                              (handler-case (%repl-code-signature llm-output)
                                                (error () llm-output))))
                                        (if (gethash signature seen-code-signatures)
                                            (values "(:error \"repeated form; use the result, choose a different call, or respond\")"
                                                    t)
                                            (progn
                                              (setf (gethash signature seen-code-signatures) t)
                                              (handler-case (%eval-all-forms llm-output)
                                                (error (e)
                                                  (%log :warn "sexp-eval" "REPL ~D: eval failed: ~A" round e)
                                                  (values nil t))))))
                                    (setf normal-eval-p t
                                          eval-result result
                                          errored result-errored)
                                    nil))))
                          (if normal-eval-p
                              (if (and eval-result (plusp (length eval-result)) (not errored))
                                  (progn
                                    (setf last-eval-result eval-result)
                                    (when (or (not (%repl-observation-only-p llm-output))
                                              (%repl-explicit-observation-request-p user-text llm-output))
                                      (setf last-presentable-result eval-result
                                            completion-model used-model
                                            completion-prompt round-prompt
                                            completed-p t))
                                    (%pipeline-trace :repl-sexp-eval-ok
                                      :round round :model used-model
                                      :eval-result (%clip-prompt eval-result 300))
                                    (%record-repl-perf used-model :code-ok :latency-ms latency-ms))
                                  (progn
                                    (%pipeline-trace :repl-sexp-eval-fail
                                      :round round :model used-model
                                      :sexp-attempted (%clip-prompt llm-output 200))
                                    (%record-repl-perf used-model :code-error :latency-ms latency-ms)
                                    (setf last-eval-result
                                          (or eval-result
                                              (format nil "(:eval-error \"~A\")"
                                                      (%clip-prompt llm-output 100))))))
                              (let ((response (if (stringp responded)
                                                  responded
                                                  (princ-to-string responded))))
                                (setf completion-model used-model
                                      completion-prompt round-prompt
                                      completed-p (%repl-usable-response-p response))
                                (%pipeline-trace :repl-sexp-eval-ok
                                  :round round :model used-model
                                  :eval-result (%clip-prompt response 300))
                                (%record-repl-perf used-model :code-ok :latency-ms latency-ms)
                                (return response))))))

                     (t
                      (%record-repl-perf used-model :natural :latency-ms latency-ms)
                      (%log :info "sexp-eval" "REPL ~D: response (~D chars)" round (length llm-output))
                      (%pipeline-trace :repl-round :round round :model used-model
                        :response-type "natural-language" :response-len (length llm-output))
                      (setf completion-model used-model
                            completion-prompt round-prompt
                            completed-p (%repl-usable-response-p llm-output))
                      (return llm-output)))))
               finally
                  (return (or (and last-presentable-result
                                   (not (%error-form-p last-presentable-result))
                                   last-presentable-result)
                              "I could not complete that within the available steps."))))
           (raw-answer (if (stringp answer) answer (princ-to-string answer)))
           (clean (%repl-auto-store-and-return user-text raw-answer))
           (success (and completed-p (%repl-usable-response-p clean)))
           (outcome-model (if success completion-model attempted-model))
           (outcome-prompt (if success completion-prompt attempted-prompt))
           (recorded-p (%record-repl-completion
                        user-text clean outcome-model outcome-prompt total-latency-ms success)))
      (when success
        (%pipeline-trace :response-delivery
          :frontend (if (harmonia-signal-p prompt) (harmonia-signal-frontend prompt) "tui")
          :response-len (length clean) :model outcome-model :latency-ms total-latency-ms))
      (values clean
              (list :outcome-recorded-p recorded-p
                    :model outcome-model
                    :llm-calls round
                    :latency-ms total-latency-ms
                    :success success
                    :model-input-prompt outcome-prompt)))))

(defun %repl-diagnostic-envelope-p (text)
  "True when TEXT is wholly a raw tool-diagnostic envelope such as
`(search: no results …)` or `(grep: no results)` — a colon-tagged observation a
primitive emits, never a user-facing answer. Structural, not name-matched: one
parenthesized form whose head is a bare lowercase word immediately followed by
':'. Distinct from prose, from real calls like `(search \"x\")` (no colon), and
from keyword plists like `(:status …)` (colon right after the paren)."
  (let ((s (string-trim '(#\Space #\Newline #\Tab) (or text ""))))
    (and (> (length s) 3)
         (char= (char s 0) #\()
         (char= (char s (1- (length s))) #\))
         (let ((colon (position #\: s)))
           (and colon (> colon 1)
                (loop for i from 1 below colon
                      for c = (char s i)
                      always (or (char<= #\a c #\z) (char= c #\-))))))))

(defun %sanitize-repl-response (response)
  "Strip REPL framing that leaked into the response. Structural only:
   removes ;; comment lines (REPL frame echo) and converts a bare tool-diagnostic
   envelope (e.g. `(search: no results …)`) into a graceful line, so no internal
   observation ever reaches the user as the answer. No agent-name matching."
  (if (and response (stringp response))
      (let ((cleaned response))
        ;; Strip leading ;; comment lines (REPL framing echo)
        (loop while (and (> (length cleaned) 3)
                         (string= (subseq cleaned 0 2) ";;"))
              do (let ((nl (position #\Newline cleaned)))
                   (if nl
                       (setf cleaned (string-trim '(#\Space #\Newline) (subseq cleaned (1+ nl))))
                       (return))))
        (cond
          ((%repl-diagnostic-envelope-p cleaned) "I couldn't find anything relevant for that.")
          ((> (length cleaned) 0) cleaned)
          (t response)))
      response))

(defun %repl-auto-store-and-return (user-text response)
  "Sanitize response, store interaction, return clean response.
   Length filtering happens once, in %memory-should-store-p — duplicating
   it here was masking real interactions whose response was terse (e.g.
   'Fact stored.'). The Q+A formatting pads the entry past the floor."
  (let ((clean (%sanitize-repl-response response)))
    (when (and clean (stringp clean) (plusp (length clean)))
      (handler-case
          (progn
            (memory-put :interaction
                        (format nil "Q: ~A~%A: ~A" user-text (%clip-prompt clean 500))
                        :tags '(:repl :interaction))
            (%pipeline-trace :memory-auto-store
              :query-len (length user-text)
              :response-len (length clean)))
        (error () nil)))
    clean))
