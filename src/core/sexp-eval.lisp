;;; sexp-eval.lisp — Safe S-expression evaluator for the LLM REPL.
;;;
;;; The LLM communicates with the agent via s-expressions. Each expression
;;; is parsed, validated against a whitelist, and executed. Results feed
;;; back to the LLM for the next round of reasoning.
;;;
;;; Design principles:
;;;   - NO TIMEOUTS. Each LLM call takes as long as it needs.
;;;     If the task is hard, use a better model, not a deadline.
;;;   - MINIMIZE TOKENS. Each round sends only what's new, not the full history.
;;;   - COMPLEXITY-DRIVEN MODEL. Hard tasks get premium models. Simple tasks get cheap ones.
;;;   - MICRO-TASKS. Each round is a focused question, not a monologue.
;;;
;;; Security: WHITELIST ONLY. No eval, no load, no arbitrary code.
;;; *read-eval* is always nil. Only 10 operations are permitted.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONSTANTS
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *sexp-eval-max-result-chars* 1500
  "Maximum characters in a single eval result. Keeps tokens lean.")

(defparameter *sexp-eval-max-rounds* 5
  "Maximum REPL rounds per user query. Each round is a micro-task.")

;;; ═══════════════════════════════════════════════════════════════════════
;;; DETECTION — is LLM output code or prose?
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Does the LLM output contain s-expression code to evaluate?
Lines starting with ( that match known operations are code."
  (when (and text (stringp text) (> (length text) 0))
    (let ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text)))
      (and (> (length trimmed) 2)
           (char= (char trimmed 0) #\()
           (%extract-sexp-ops trimmed)))))

(defun %extract-sexp-ops (text)
  "Extract all s-expression operations from text. Returns list of form strings."
  (let ((ops '())
        (start 0))
    (loop while (< start (length text)) do
      (let ((paren-start (position #\( text :start start)))
        (if (null paren-start)
            (return)
            (let ((paren-end (%find-matching-paren text paren-start)))
              (if paren-end
                  (let* ((form-str (subseq text paren-start (1+ paren-end)))
                         (*read-eval* nil)
                         (form (ignore-errors (read-from-string form-str))))
                    (when (and (listp form) (symbolp (car form))
                               (member (car form) '(recall recall-verbatim store status
                                                    tools tool introspect signalograd
                                                    basin source)))
                      (push form-str ops))
                    (setf start (1+ paren-end)))
                  (return))))))
    (nreverse ops)))

(defun %find-matching-paren (text start)
  "Find the index of the matching closing paren."
  (let ((depth 0)
        (in-string nil))
    (loop for i from start below (length text) do
      (let ((ch (char text i)))
        (cond
          ((and (char= ch #\") (not in-string)) (setf in-string t))
          ((and (char= ch #\") in-string) (setf in-string nil))
          ((and (char= ch #\() (not in-string)) (incf depth))
          ((and (char= ch #\)) (not in-string))
           (decf depth)
           (when (zerop depth) (return-from %find-matching-paren i))))))
    nil))

;;; ═══════════════════════════════════════════════════════════════════════
;;; SAFE EVALUATOR — whitelist only, never arbitrary eval
;;; ═══════════════════════════════════════════════════════════════════════

(defun %eval-safe-sexp (form-string)
  "Parse and execute a whitelisted s-expression. Returns result string.
NEVER calls eval. Only dispatches to known safe functions."
  (handler-case
      (let* ((*read-eval* nil)
             (form (read-from-string form-string)))
        (if (and (listp form) (symbolp (car form)))
            (let ((op (car form))
                  (args (cdr form)))
              (%bound-result
               (case op
                 (recall          (%safe-recall (first args)))
                 (recall-verbatim (%safe-recall-verbatim (first args)))
                 (store           (%safe-store (first args) (rest args)))
                 (status          (%safe-status))
                 (tools           (%safe-tools))
                 (tool            (%safe-tool (first args) (rest args)))
                 (introspect      (%safe-introspect))
                 (signalograd     (%safe-signalograd))
                 (basin           (%safe-basin))
                 (source          (%safe-source (first args)))
                 (otherwise       (format nil "(:error \"unknown operation: ~A\")" op)))))
            (format nil "(:error \"not a valid s-expression\")")))
    (error (c)
      (format nil "(:error \"eval failed: ~A\")" (princ-to-string c)))))

(defun %eval-all-sexps (text)
  "Evaluate all s-expression operations in text. Returns combined results."
  (let* ((ops (%extract-sexp-ops text))
         (results (mapcar (lambda (op)
                            (%log :info "sexp-eval" "Eval: ~A"
                                  (subseq op 0 (min 80 (length op))))
                            (let ((result (%eval-safe-sexp op)))
                              (format nil "~A → ~A" op result)))
                          ops)))
    (format nil "~{~A~%~}" results)))

(defun %bound-result (text)
  "Bound a result string to *sexp-eval-max-result-chars*."
  (let ((s (if (stringp text) text (princ-to-string text))))
    (if (> (length s) *sexp-eval-max-result-chars*)
        (concatenate 'string
                     (subseq s 0 *sexp-eval-max-result-chars*)
                     "... [truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; SAFE OPERATION IMPLEMENTATIONS
;;; ═══════════════════════════════════════════════════════════════════════

(defun %safe-recall (query)
  "Search memory field by resonance."
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (ignore-errors
          (let ((results (memory-semantic-recall-block q :limit 5 :max-chars 1200)))
            (if (and results (> (length results) 0))
                results
                "(no memories found)")))
        "(memory recall unavailable)")))

(defun %safe-recall-verbatim (name)
  "Exact match recall for skills, instructions, source code."
  (let ((n (if (stringp name) name (princ-to-string name))))
    (or (ignore-errors
          (let ((entries (when (fboundp 'memory-recall-verbatim)
                           (funcall 'memory-recall-verbatim n))))
            (if entries
                (format nil "~{~A~%~}"
                        (mapcar (lambda (e)
                                  (let ((c (memory-entry-content e)))
                                    (if (and (listp c) (getf c :content))
                                        (getf c :content)
                                        (%entry-text e))))
                                entries))
                "(no verbatim match)")))
        "(verbatim recall unavailable)")))

(defun %safe-store (content rest-args)
  "Store a new memory entry."
  (let ((text (if (stringp content) content (princ-to-string content)))
        (tags (getf rest-args :tags)))
    (ignore-errors
      (memory-put :daily text :tags (or tags '(:user-stored))))
    "(:ok stored)"))

(defun %safe-status ()
  "System health snapshot."
  (let ((field (ignore-errors
                 (when (and (fboundp 'memory-field-port-ready-p)
                            (funcall 'memory-field-port-ready-p))
                   (ipc-call "(:component \"memory-field\" :op \"status\")"))))
        (health (ignore-errors
                  (ipc-call "(:component \"provider-router\" :op \"healthcheck\")"))))
    (format nil "field: ~A~%router: ~A"
            (or field "unavailable")
            (or health "unavailable"))))

(defun %safe-tools ()
  "List available tools."
  (let ((tools (ignore-errors
                 (when (boundp '*runtime*)
                   (let ((names '()))
                     (maphash (lambda (k _) (declare (ignore _)) (push k names))
                              (runtime-state-tools *runtime*))
                     names)))))
    (format nil "(:tools ~{\"~A\"~^ ~})" (or tools '("none loaded")))))

(defun %safe-tool (name rest-args)
  "Execute a registered tool via the existing tool dispatch."
  (let ((tool-name (if (stringp name) name (princ-to-string name))))
    (or (ignore-errors
          (let ((cmd (format nil "tool op=~A~{ ~A~}"
                             tool-name
                             (loop for (k v) on rest-args by #'cddr
                                   collect (format nil "~A=~A"
                                                   (string-downcase (symbol-name k))
                                                   v)))))
            (when (fboundp '%maybe-handle-tool-command)
              (funcall '%maybe-handle-tool-command cmd))))
        (format nil "(:error \"tool ~A not found or failed\")" tool-name))))

(defun %safe-introspect ()
  "Runtime self-diagnosis."
  (or (ignore-errors
        (when (fboundp 'introspect-runtime)
          (let ((result (funcall 'introspect-runtime)))
            (if (stringp result) result (princ-to-string result)))))
      "(introspection unavailable)"))

(defun %safe-signalograd ()
  "Adaptive kernel state."
  (or (ignore-errors
        (ipc-call "(:component \"signalograd\" :op \"status\")"))
      "(signalograd unavailable)"))

(defun %safe-basin ()
  "Current memory field basin status."
  (or (ignore-errors
        (when (and (fboundp 'memory-field-port-ready-p)
                   (funcall 'memory-field-port-ready-p))
          (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))
      "(basin unavailable)"))

(defun %safe-source (query)
  "Search source code in memory field."
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (ignore-errors
          (let ((verbatim (%safe-recall-verbatim q)))
            (when (and verbatim (not (search "no verbatim" verbatim)))
              verbatim)))
        (%safe-recall (format nil "source code ~A" q)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; COMPLEXITY-DRIVEN MODEL SELECTION
;;; ═══════════════════════════════════════════════════════════════════════

(defun %repl-select-model (user-text)
  "Select model based on task complexity. Hard tasks get premium models.
Uses the existing complexity encoder via the router, but biases toward
better models for orchestration-level reasoning."
  (or (ignore-errors
        ;; Try the existing model selection which uses complexity encoding.
        (when (fboundp '%select-model)
          (let ((model (funcall '%select-model user-text)))
            ;; If the selected model is a free/tiny model, upgrade for orchestration.
            (if (and model
                     (or (search ":free" model)
                         (search "mercury" model)
                         (search "nano" model)))
                ;; Upgrade: pick from seed models (configured premium models).
                (when (fboundp '%seed-models)
                  (let ((seeds (funcall '%seed-models)))
                    (or (find-if (lambda (m)
                                   (and (not (search ":free" m))
                                        (not (search "mercury" m))))
                                 seeds)
                        model)))
                model))))
      ;; Fallback: let router decide.
      "auto"))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE REPL LOOP — no timeouts, micro-tasks, lean context
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *sexp-eval-max-rounds*))
  "Multi-round REPL: LLM outputs code → agent evaluates → feeds back → repeat.

Design:
  - NO TIMEOUTS. Each LLM call takes as long as it needs.
  - LEAN CONTEXT. Each round sends only: bootstrap + latest eval result + user question.
    NOT the full accumulated conversation. This minimizes tokens.
  - COMPLEXITY MODEL. Harder tasks get better models via %repl-select-model.
  - MICRO-TASKS. The LLM can execute multiple s-expressions per round."
  (let* ((bootstrap (dna-system-prompt :mode :orchestrate))
         ;; Seed with automatic memory recall for context.
         (user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-text prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         (initial-recall (ignore-errors
                           (memory-semantic-recall-block user-text
                             :limit 5 :max-chars 1000)))
         ;; Select model based on complexity.
         (model (%repl-select-model user-text))
         ;; Build the first prompt: bootstrap + recall + question.
         (current-context
           (format nil "~A~:[~;~%~%RECALLED_CONTEXT:~%~A~]~%~%USER: ~A"
                   bootstrap
                   (and initial-recall (> (length initial-recall) 0))
                   initial-recall
                   user-text))
         (round 0)
         (last-eval-result nil))

    (%log :info "sexp-eval" "REPL start: model=~A user=[~A]"
          model (subseq user-text 0 (min 60 (length user-text))))

    (loop while (< round max-rounds) do
      (incf round)

      ;; Build this round's prompt. After round 1, send ONLY the eval result + reminder.
      ;; This keeps tokens minimal — no accumulated conversation bloat.
      (let ((round-prompt
              (if (= round 1)
                  current-context
                  ;; Subsequent rounds: lean. Only bootstrap + eval result + reminder.
                  (format nil "~A~%~%EVAL_RESULT from your previous code:~%~A~%~%The user asked: ~A~%~%Continue: execute more code or respond in natural language."
                          bootstrap
                          (or last-eval-result "(no result)")
                          user-text))))

        ;; Call LLM — NO DEADLINE. Let it think as long as it needs.
        (let ((llm-output
                (handler-case
                    (backend-complete round-prompt (or model "auto"))
                  (error (c)
                    (%log :warn "sexp-eval" "REPL round ~D LLM error: ~A" round c)
                    nil))))

          (cond
            ;; LLM failed — return fallback.
            ((null llm-output)
             (%log :info "sexp-eval" "REPL round ~D: LLM unavailable" round)
             (return-from %orchestrate-repl nil))

            ;; LLM output contains s-expression code → evaluate and loop.
            ((%is-sexp-output-p llm-output)
             (%log :info "sexp-eval" "REPL round ~D: evaluating s-expressions" round)
             (setf last-eval-result (%eval-all-sexps llm-output)))

            ;; LLM output is natural language → that's the response.
            (t
             (%log :info "sexp-eval" "REPL round ~D: natural language (~D chars)"
                   round (length llm-output))
             (return-from %orchestrate-repl llm-output))))))

    ;; Exceeded max rounds — this shouldn't happen often with lean context.
    (%log :warn "sexp-eval" "REPL exceeded ~D rounds" max-rounds)
    nil))
