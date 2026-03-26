;;; sexp-eval.lisp — Safe S-expression evaluator for the LLM REPL.
;;;
;;; The LLM communicates with the agent via s-expressions. Each expression
;;; is parsed, validated against a whitelist, and executed. Results feed
;;; back to the LLM for the next round of reasoning.
;;;
;;; Design principles:
;;;   - NO TIMEOUTS. User interrupts with ESC. LLM takes as long as needed.
;;;   - PARALLEL RECALL. Memory, basin, and status fetched simultaneously
;;;     via actors before the first LLM call. Zero wasted wait time.
;;;   - COMPLEXITY-DRIVEN MODEL. Encoder scores the prompt. Simple → cheap.
;;;     Hard → premium. Multiple cheap calls beat one expensive one.
;;;   - MINIMIZE TOKENS. Each round sends only what's new. Lean micro-tasks.
;;;   - WHITELIST ONLY. 10 safe operations. No eval, no load, no code execution.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONSTANTS
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *sexp-eval-max-result-chars* 1500)
(defparameter *sexp-eval-max-rounds* 5)

;;; ═══════════════════════════════════════════════════════════════════════
;;; PARALLEL RECALL — fire all queries simultaneously via actors
;;; ═══════════════════════════════════════════════════════════════════════

(defun %parallel-gather-context (query)
  "Gather memory recall, basin status, and system health in PARALLEL.
Spawns 3 lightweight actors, collects results. Total wall-time = max(individual),
not sum(individual). Returns plist with :recall :basin :health."
  (let* ((recall-result nil)
         (basin-result nil)
         ;; Use threads for true parallelism (SBCL supports native threads).
         (recall-thread
           (sb-thread:make-thread
            (lambda ()
              (setf recall-result
                    (ignore-errors
                      (memory-semantic-recall-block query :limit 5 :max-chars 1000))))
            :name "parallel-recall"))
         (basin-thread
           (sb-thread:make-thread
            (lambda ()
              (setf basin-result
                    (ignore-errors
                      (when (and (fboundp 'memory-field-port-ready-p)
                                 (funcall 'memory-field-port-ready-p))
                        (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))))
            :name "parallel-basin")))
    ;; Wait for both threads (they run in parallel).
    (ignore-errors (sb-thread:join-thread recall-thread))
    (ignore-errors (sb-thread:join-thread basin-thread))
    ;; Return collected results.
    (list :recall (or recall-result "")
          :basin (or basin-result ""))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; COMPLEXITY-DRIVEN MODEL SELECTION
;;; ═══════════════════════════════════════════════════════════════════════

(defun %repl-select-model (user-text)
  "Select model based on task complexity via the encoder.
Simple → cheapest. Medium → auto. Complex/Reasoning → premium seed model.
Multiple cheap calls are better than one expensive call."
  (let* (;; Use complexity encoder via IPC (it's in the Rust runtime).
         (profile (ignore-errors
                    (when (fboundp '%select-model)
                      (funcall '%select-model user-text))))
         ;; Check if the auto-selected model is too cheap for orchestration.
         (model (or profile "auto")))
    ;; If the model is free/nano/mercury, upgrade for orchestration reasoning.
    (when (and (stringp model)
               (or (search ":free" model)
                   (search "mercury" model)
                   (search "nano" model)))
      (setf model
            (or (ignore-errors
                  (when (fboundp '%seed-models)
                    (let ((seeds (funcall '%seed-models)))
                      (find-if (lambda (m)
                                 (and (not (search ":free" m))
                                      (not (search "mercury" m))))
                               seeds))))
                model)))
    model))

;;; ═══════════════════════════════════════════════════════════════════════
;;; DETECTION — is LLM output code or prose?
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Does the LLM output contain s-expression code to evaluate?"
  (when (and text (stringp text) (> (length text) 0))
    (let ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text)))
      (and (> (length trimmed) 2)
           (char= (char trimmed 0) #\()
           (%extract-sexp-ops trimmed)))))

(defun %extract-sexp-ops (text)
  "Extract all s-expression operations from text."
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
  (let ((depth 0) (in-string nil))
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
;;; SAFE EVALUATOR — whitelist only
;;; ═══════════════════════════════════════════════════════════════════════

(defun %eval-safe-sexp (form-string)
  "Parse and execute a whitelisted s-expression. NEVER calls eval."
  (handler-case
      (let* ((*read-eval* nil)
             (form (read-from-string form-string)))
        (if (and (listp form) (symbolp (car form)))
            (let ((op (car form)) (args (cdr form)))
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
                 (otherwise       (format nil "(:error \"unknown: ~A\")" op)))))
            "(:error \"not a valid s-expression\")"))
    (error (c) (format nil "(:error \"~A\")" (princ-to-string c)))))

(defun %eval-all-sexps (text)
  "Evaluate all s-expression operations in text."
  (let ((ops (%extract-sexp-ops text)))
    (format nil "~{~A~%~}"
            (mapcar (lambda (op)
                      (%log :info "sexp-eval" "Eval: ~A" (subseq op 0 (min 80 (length op))))
                      (format nil "~A → ~A" op (%eval-safe-sexp op)))
                    ops))))

(defun %bound-result (text)
  (let ((s (if (stringp text) text (princ-to-string text))))
    (if (> (length s) *sexp-eval-max-result-chars*)
        (concatenate 'string (subseq s 0 *sexp-eval-max-result-chars*) "...[truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; SAFE OPERATIONS
;;; ═══════════════════════════════════════════════════════════════════════

(defun %safe-recall (query)
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (ignore-errors
          (let ((r (memory-semantic-recall-block q :limit 5 :max-chars 1200)))
            (if (and r (> (length r) 0)) r "(no memories found)")))
        "(memory recall unavailable)")))

(defun %safe-recall-verbatim (name)
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
  (let ((text (if (stringp content) content (princ-to-string content)))
        (tags (getf rest-args :tags)))
    (ignore-errors (memory-put :daily text :tags (or tags '(:user-stored))))
    "(:ok stored)"))

(defun %safe-status ()
  (let ((field (ignore-errors
                 (when (and (fboundp 'memory-field-port-ready-p)
                            (funcall 'memory-field-port-ready-p))
                   (ipc-call "(:component \"memory-field\" :op \"status\")"))))
        (health (ignore-errors
                  (ipc-call "(:component \"provider-router\" :op \"healthcheck\")"))))
    (format nil "field: ~A~%router: ~A" (or field "unavailable") (or health "unavailable"))))

(defun %safe-tools ()
  (let ((tools (ignore-errors
                 (when (boundp '*runtime*)
                   (let ((names '()))
                     (maphash (lambda (k _) (declare (ignore _)) (push k names))
                              (runtime-state-tools *runtime*))
                     names)))))
    (format nil "(:tools ~{\"~A\"~^ ~})" (or tools '("none loaded")))))

(defun %safe-tool (name rest-args)
  (let ((tool-name (if (stringp name) name (princ-to-string name))))
    (or (ignore-errors
          (let ((cmd (format nil "tool op=~A~{ ~A~}" tool-name
                             (loop for (k v) on rest-args by #'cddr
                                   collect (format nil "~A=~A"
                                                   (string-downcase (symbol-name k)) v)))))
            (when (fboundp '%maybe-handle-tool-command)
              (funcall '%maybe-handle-tool-command cmd))))
        (format nil "(:error \"tool ~A failed\")" tool-name))))

(defun %safe-introspect ()
  (or (ignore-errors
        (when (fboundp 'introspect-runtime)
          (let ((r (funcall 'introspect-runtime)))
            (if (stringp r) r (princ-to-string r)))))
      "(introspection unavailable)"))

(defun %safe-signalograd ()
  (or (ignore-errors (ipc-call "(:component \"signalograd\" :op \"status\")"))
      "(signalograd unavailable)"))

(defun %safe-basin ()
  (or (ignore-errors
        (when (and (fboundp 'memory-field-port-ready-p)
                   (funcall 'memory-field-port-ready-p))
          (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))
      "(basin unavailable)"))

(defun %safe-source (query)
  (let ((q (if (stringp query) query (princ-to-string query))))
    (or (ignore-errors
          (let ((v (%safe-recall-verbatim q)))
            (when (and v (not (search "no verbatim" v))) v)))
        (%safe-recall (format nil "source code ~A" q)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE REPL LOOP — parallel recall, no timeouts, lean micro-rounds
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *sexp-eval-max-rounds*))
  "Multi-round REPL with parallel context gathering and complexity-driven models.

Flow:
  1. PARALLEL: gather memory recall + basin status simultaneously (actors)
  2. Complexity encoder selects model (cheap for simple, premium for hard)
  3. Send lean prompt to LLM (bootstrap + parallel results + question)
  4. If LLM outputs s-expressions → evaluate → feed back (micro-round)
  5. If LLM outputs natural language → that's the response
  6. NO TIMEOUTS. User interrupts with ESC."
  (let* ((user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-text prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         (bootstrap (dna-system-prompt :mode :orchestrate))
         ;; PARALLEL: gather all context simultaneously.
         (context (%parallel-gather-context user-text))
         (recall (getf context :recall))
         (basin (getf context :basin))
         ;; Complexity-driven model selection.
         (model (%repl-select-model user-text))
         ;; Build first prompt: bootstrap + parallel results + question.
         (current-prompt
           (format nil "~A~:[~;~%~%RECALLED_CONTEXT:~%~A~]~:[~;~%~%BASIN: ~A~]~%~%USER: ~A"
                   bootstrap
                   (and recall (> (length recall) 0)) recall
                   (and basin (> (length basin) 0)) basin
                   user-text))
         (round 0)
         (last-eval-result nil))

    (%log :info "sexp-eval" "REPL: model=~A prompt-len=~D user=[~A]"
          model (length current-prompt) (subseq user-text 0 (min 60 (length user-text))))

    (loop while (< round max-rounds) do
      (incf round)
      ;; Build this round's prompt. Lean — no accumulated conversation.
      (let ((round-prompt
              (if (= round 1)
                  current-prompt
                  ;; Micro-round: only bootstrap + eval result + reminder.
                  (format nil "~A~%~%EVAL_RESULT:~%~A~%~%USER asked: ~A~%~%Respond naturally or execute more code."
                          bootstrap (or last-eval-result "") user-text))))

        ;; Call LLM — NO TIMEOUT. User interrupts with ESC in TUI.
        (let ((llm-output
                (handler-case
                    (backend-complete round-prompt (or model "auto"))
                  (error (c)
                    (%log :warn "sexp-eval" "REPL round ~D error: ~A" round c)
                    nil))))
          (cond
            ((null llm-output)
             (%log :info "sexp-eval" "REPL round ~D: LLM unavailable" round)
             (return-from %orchestrate-repl nil))

            ((%is-sexp-output-p llm-output)
             (%log :info "sexp-eval" "REPL round ~D: eval s-expressions" round)
             (setf last-eval-result (%eval-all-sexps llm-output)))

            (t
             (%log :info "sexp-eval" "REPL round ~D: response (~D chars)" round (length llm-output))
             (return-from %orchestrate-repl llm-output))))))

    (%log :warn "sexp-eval" "REPL exceeded ~D rounds" max-rounds)
    nil))
