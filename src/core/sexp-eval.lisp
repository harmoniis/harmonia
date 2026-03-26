;;; sexp-eval.lisp — Safe S-expression evaluator for the LLM REPL.
;;;
;;; The LLM communicates with the agent via s-expressions. Each expression
;;; is parsed, validated against a whitelist, and executed. Results feed
;;; back to the LLM for the next round of reasoning.
;;;
;;; Security: WHITELIST ONLY. No eval, no load, no arbitrary code.
;;; *read-eval* is always nil. Only 10 operations are permitted.
;;; Results are bounded (max 2000 chars). All evals are logged.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONSTANTS
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *sexp-eval-max-result-chars* 2000
  "Maximum characters in a single eval result.")

(defparameter *sexp-eval-max-rounds* 5
  "Maximum REPL rounds per user query.")

;;; ═══════════════════════════════════════════════════════════════════════
;;; DETECTION — is LLM output code or prose?
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Does the LLM output contain s-expression code to evaluate?
Lines starting with ( are code. Pure prose has none."
  (when (and text (stringp text) (> (length text) 0))
    (let ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text)))
      (and (> (length trimmed) 2)
           (char= (char trimmed 0) #\()
           ;; Must be a known operation, not just any parenthesized text.
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
                            (%log :info "sexp-eval" "Evaluating: ~A"
                                  (subseq op 0 (min 80 (length op))))
                            (let ((result (%eval-safe-sexp op)))
                              ;; Log to Chronicle
                              (ignore-errors
                                (ipc-call
                                 (format nil "(:component \"chronicle\" :op \"record-ouroboros-event\" :event-type \"sexp-eval\" :generation 0 :fitness 0.0 :mutation-count 0 :crossover-count 0 :detail \"~A\")"
                                         (sexp-escape-lisp (subseq op 0 (min 60 (length op)))))))
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
          (let ((results (memory-semantic-recall-block q :limit 5 :max-chars 1800)))
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
                 (when (fboundp 'memory-field-port-ready-p)
                   (if (funcall 'memory-field-port-ready-p)
                       (ipc-call "(:component \"memory-field\" :op \"status\")")
                       "field: not ready"))))
        (sig (ignore-errors
               (ipc-call "(:component \"signalograd\" :op \"status\")")))
        (health (ignore-errors
                  (ipc-call "(:component \"provider-router\" :op \"healthcheck\")"))))
    (format nil "SYSTEM_STATUS:~%field: ~A~%signalograd: ~A~%router: ~A"
            (or field "unavailable")
            (or (and sig (subseq sig 0 (min 120 (length sig)))) "unavailable")
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
    ;; Delegate to existing tool system
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
    ;; Try verbatim recall first (exact file path match)
    (or (ignore-errors
          (let ((verbatim (%safe-recall-verbatim q)))
            (when (and verbatim (not (search "no verbatim" verbatim)))
              verbatim)))
        ;; Fall back to field recall for concept-level search
        (%safe-recall (format nil "source code ~A" q)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE REPL LOOP — multi-round LLM ↔ code execution
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *sexp-eval-max-rounds*))
  "Multi-round REPL: LLM outputs code → agent evaluates → feeds back → repeat.
LLM receives minimal bootstrap. When it outputs s-expressions, they're evaluated.
When it outputs natural language, that's the response to the user."
  (let* ((bootstrap (dna-system-prompt :mode :orchestrate))
         ;; Seed with automatic memory recall for context
         (initial-recall (ignore-errors
                           (memory-semantic-recall-block
                            (if (harmonia-signal-p prompt)
                                (harmonia-signal-text prompt)
                                prompt)
                            :limit 3 :max-chars 800)))
         (user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-text prompt)
                        prompt))
         (conversation
           (format nil "~A~:[~;~%~%RECALLED_CONTEXT:~%~A~]~%~%USER: ~A"
                   bootstrap
                   (and initial-recall (> (length initial-recall) 0))
                   initial-recall
                   user-text))
         (round 0)
         (model (ignore-errors (%select-model user-text))))

    (loop while (< round max-rounds) do
      (incf round)
      (let ((llm-output
              (handler-case
                  (sb-sys:with-deadline (:seconds 45)
                    (backend-complete conversation (or model "auto")))
                (sb-sys:deadline-timeout ()
                  (%log :warn "sexp-eval" "LLM timeout in REPL round ~D" round)
                  nil)
                (error (c)
                  (%log :warn "sexp-eval" "LLM error in REPL round ~D: ~A" round c)
                  nil))))

        (cond
          ;; LLM failed — return what we have
          ((null llm-output)
           (%log :info "sexp-eval" "REPL: LLM unavailable at round ~D, returning fallback" round)
           (return-from %orchestrate-repl
             (or (when (> round 1)
                   "I was exploring your question but lost connection to my reasoning engine. Let me try again.")
                 nil)))

          ;; LLM output contains s-expression code → evaluate and loop
          ((%is-sexp-output-p llm-output)
           (%log :info "sexp-eval" "REPL round ~D: evaluating s-expressions" round)
           (let ((eval-results (%eval-all-sexps llm-output)))
             ;; Append the exchange to conversation
             (setf conversation
                   (format nil "~A~%~%AGENT_CODE:~%~A~%~%EVAL_RESULT:~%~A"
                           conversation llm-output eval-results))))

          ;; LLM output is natural language → that's the response
          (t
           (%log :info "sexp-eval" "REPL round ~D: natural language response (~D chars)"
                 round (length llm-output))
           (return-from %orchestrate-repl llm-output)))))

    ;; Exceeded max rounds without natural language response
    (%log :warn "sexp-eval" "REPL exceeded ~D rounds without final response" max-rounds)
    "I explored extensively but need to think more. Let me summarize what I found so far."))
