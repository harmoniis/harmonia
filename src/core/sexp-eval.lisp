;;; sexp-eval.lisp — Generic safe S-expression evaluator.
;;;
;;; The LLM speaks to the agent via s-expressions. Three generic operations
;;; handle everything — no hardcoded cases:
;;;
;;;   (ipc "component" "op" :key val ...)  — call any IPC component
;;;   (recall "query" :limit N)            — search memory field
;;;   (tool "name" :key val ...)           — execute any registered tool
;;;
;;; That's it. Three verbs. The LLM discovers what components, tools,
;;; and memories exist by calling them. The system is the documentation.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONSTANTS
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *sexp-eval-max-result-chars* 1500)
(defparameter *sexp-eval-max-rounds* 5)

;;; ═══════════════════════════════════════════════════════════════════════
;;; PARALLEL CONTEXT GATHERING
;;; ═══════════════════════════════════════════════════════════════════════

(defun %parallel-gather-context (query)
  "Gather memory recall + basin status in parallel via threads."
  (let* ((recall-result nil)
         (basin-result nil)
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
    (ignore-errors (sb-thread:join-thread recall-thread))
    (ignore-errors (sb-thread:join-thread basin-thread))
    (list :recall (or recall-result "")
          :basin (or basin-result ""))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; MODEL SELECTION
;;; ═══════════════════════════════════════════════════════════════════════

(defun %repl-select-model (user-text)
  "Complexity-driven model selection. Simple → cheap. Hard → premium."
  (or (ignore-errors
        (when (fboundp '%select-model)
          (let ((model (funcall '%select-model user-text)))
            (if (and (stringp model)
                     (or (search ":free" model)
                         (search "mercury" model)
                         (search "nano" model)))
                (ignore-errors
                  (when (fboundp '%seed-models)
                    (let ((seeds (funcall '%seed-models)))
                      (or (find-if (lambda (m)
                                     (and (not (search ":free" m))
                                          (not (search "mercury" m))))
                                   seeds)
                          model))))
                model))))
      "auto"))

;;; ═══════════════════════════════════════════════════════════════════════
;;; DETECTION — is LLM output code or prose?
;;; ═══════════════════════════════════════════════════════════════════════

(defun %is-sexp-output-p (text)
  "Does the LLM output look like s-expression code?
Simple: starts with ( and contains known verb."
  (when (and text (stringp text) (> (length text) 2))
    (let* ((trimmed (string-trim '(#\Space #\Newline #\Return #\Tab) text))
           (lower (string-downcase trimmed)))
      (and (> (length trimmed) 2)
           (char= (char trimmed 0) #\()
           (or (search "(ipc " lower)
               (search "(recall " lower)
               (search "(tool " lower))))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENERIC SAFE EVALUATOR — three verbs handle everything
;;; ═══════════════════════════════════════════════════════════════════════

(defun %eval-safe-sexp (form-string)
  "Parse and execute a generic s-expression. Three verbs:
  (ipc \"component\" \"op\" :key val ...)  — any IPC component
  (recall \"query\" :limit N)             — memory search
  (tool \"name\" :key val ...)            — any registered tool
NEVER calls eval. Always bounded."
  (handler-case
      (let* ((*read-eval* nil)
             (form (read-from-string form-string)))
        (if (and (listp form) (symbolp (car form)))
            (%bound-result
             (case (car form)
               (ipc     (apply #'%generic-ipc (cdr form)))
               (recall  (apply #'%generic-recall (cdr form)))
               (tool    (apply #'%generic-tool (cdr form)))
               (otherwise (format nil "(:error \"unknown verb: ~A. Use: ipc, recall, tool\")"
                                  (car form)))))
            "(:error \"not a valid s-expression\")"))
    (error (c) (format nil "(:error \"~A\")" (princ-to-string c)))))

(defun %eval-all-sexps (text)
  "Evaluate all s-expressions in text. Returns combined results."
  (let ((results '())
        (start 0))
    (loop while (< start (length text)) do
      (let ((paren-start (position #\( text :start start)))
        (if (null paren-start)
            (return)
            (let ((paren-end (%find-matching-paren text paren-start)))
              (if paren-end
                  (let* ((form-str (subseq text paren-start (1+ paren-end)))
                         (result (%eval-safe-sexp form-str)))
                    (%log :info "sexp-eval" "Eval: ~A → ~D chars"
                          (subseq form-str 0 (min 60 (length form-str)))
                          (length result))
                    (push (format nil "~A → ~A" form-str result) results)
                    (setf start (1+ paren-end)))
                  ;; Truncated — try to salvage.
                  (let* ((rest (subseq text paren-start))
                         (fixed (concatenate 'string rest "\")"))
                         (result (%eval-safe-sexp fixed)))
                    (push (format nil "~A → ~A" rest result) results)
                    (return)))))))
    (format nil "~{~A~%~}" (nreverse results))))

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

(defun %bound-result (text)
  (let ((s (if (stringp text) text (princ-to-string text))))
    (if (> (length s) *sexp-eval-max-result-chars*)
        (concatenate 'string (subseq s 0 *sexp-eval-max-result-chars*) "...[truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENERIC IPC — call ANY component with ANY operation
;;; ═══════════════════════════════════════════════════════════════════════

(defun %generic-ipc (component op &rest kwargs)
  "Call any IPC component with any operation. Fully generic.
  (ipc \"memory-field\" \"status\")
  (ipc \"chronicle\" \"query\" :sql \"SELECT ...\")
  (ipc \"signalograd\" \"status\")
  (ipc \"provider-router\" \"healthcheck\")"
  (let* ((comp (if (stringp component) component (princ-to-string component)))
         (operation (if (stringp op) op (princ-to-string op)))
         ;; Build kwargs as key-value sexp pairs.
         (kv-str (with-output-to-string (out)
                   (loop for (k v) on kwargs by #'cddr do
                     (let ((key-name (if (keywordp k)
                                         (string-downcase (symbol-name k))
                                         (princ-to-string k)))
                           (val-str (if (stringp v)
                                        (format nil "\"~A\"" (sexp-escape-lisp v))
                                        (princ-to-string v))))
                       (format out " :~A ~A" key-name val-str)))))
         (sexp (format nil "(:component \"~A\" :op \"~A\"~A)"
                       (sexp-escape-lisp comp) (sexp-escape-lisp operation) kv-str))
         (reply (ipc-call sexp)))
    (or reply "(ipc: no response)")))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENERIC RECALL — search any memory layer
;;; ═══════════════════════════════════════════════════════════════════════

(defun %generic-recall (query &key (limit 5) (max-chars 1200) verbatim)
  "Search memory. Resonance by default, exact match with :verbatim t.
  (recall \"harmony attractor\")
  (recall \"conductor.lisp\" :verbatim t)
  (recall \"recent interactions\" :limit 10)"
  (let ((q (if (stringp query) query (princ-to-string query))))
    (if verbatim
        ;; Exact match recall.
        (or (ignore-errors
              (when (fboundp 'memory-recall-verbatim)
                (let ((entries (funcall 'memory-recall-verbatim q)))
                  (if entries
                      (format nil "~{~A~%~}"
                              (mapcar (lambda (e)
                                        (let ((c (memory-entry-content e)))
                                          (if (and (listp c) (getf c :content))
                                              (getf c :content)
                                              (%entry-text e))))
                                      entries))
                      "(no verbatim match)"))))
            "(verbatim recall unavailable)")
        ;; Resonance recall via memory field.
        (or (ignore-errors
              (let ((r (memory-semantic-recall-block q :limit limit :max-chars max-chars)))
                (if (and r (> (length r) 0)) r "(no memories found)")))
            "(memory recall unavailable)"))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GENERIC TOOL — execute any registered tool
;;; ═══════════════════════════════════════════════════════════════════════

(defun %generic-tool (name &rest kwargs)
  "Execute any registered tool via the existing tool dispatch.
  (tool \"search\" :query \"Lorenz attractor\")
  (tool \"gateway-list\")
  (tool \"matrix-report\")"
  (let* ((tool-name (if (stringp name) name (princ-to-string name)))
         (cmd (format nil "tool op=~A~{ ~A~}"
                      tool-name
                      (loop for (k v) on kwargs by #'cddr
                            collect (format nil "~A=~A"
                                            (string-downcase (symbol-name k))
                                            (if (stringp v) v (princ-to-string v)))))))
    (or (ignore-errors
          (when (fboundp '%maybe-handle-tool-command)
            (funcall '%maybe-handle-tool-command cmd)))
        (format nil "(:error \"tool ~A not found\")" tool-name))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE REPL LOOP
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *sexp-eval-max-rounds*))
  "Multi-round REPL. Three generic verbs: ipc, recall, tool.
Parallel context gathering. Complexity-driven model. No timeouts.
Never shows code to user — always responds in natural language."
  (let* ((user-text (if (harmonia-signal-p prompt)
                        (harmonia-signal-text prompt)
                        (if (stringp prompt) prompt (princ-to-string prompt))))
         (bootstrap (dna-system-prompt :mode :orchestrate))
         (context (%parallel-gather-context user-text))
         (recall (getf context :recall))
         (basin (getf context :basin))
         (model (%repl-select-model user-text))
         (current-prompt
           (format nil "~A~:[~;~%~%RECALLED_CONTEXT:~%~A~]~:[~;~%BASIN: ~A~]~%~%USER: ~A"
                   bootstrap
                   (and recall (> (length recall) 0)) recall
                   (and basin (> (length basin) 0)) basin
                   user-text))
         (round 0)
         (last-eval-result nil))

    (%log :info "sexp-eval" "REPL: model=~A len=~D user=[~A]"
          model (length current-prompt) (subseq user-text 0 (min 60 (length user-text))))

    (loop while (< round max-rounds) do
      (incf round)
      (let ((round-prompt
              (if (= round 1)
                  current-prompt
                  (format nil "~A~%~%EVAL_RESULT:~%~A~%~%USER asked: ~A~%Respond in natural language or execute more (ipc ...) / (recall ...) / (tool ...)."
                          bootstrap (or last-eval-result "") user-text))))

        (let ((llm-output
                (handler-case (backend-complete round-prompt (or model "auto"))
                  (error (c)
                    (%log :warn "sexp-eval" "REPL ~D error: ~A" round c)
                    nil))))
          (cond
            ((null llm-output)
             (%log :info "sexp-eval" "REPL ~D: LLM unavailable" round)
             (return-from %orchestrate-repl nil))

            ;; Output starts with ( → it's code. Evaluate, never show to user.
            ((and (> (length llm-output) 3)
                  (char= (char (string-trim '(#\Space #\Newline) llm-output) 0) #\())
             (%log :info "sexp-eval" "REPL ~D: eval code" round)
             (setf last-eval-result (%eval-all-sexps llm-output)))

            ;; Natural language → return to user.
            (t
             (%log :info "sexp-eval" "REPL ~D: response (~D chars)" round (length llm-output))
             (return-from %orchestrate-repl llm-output))))))

    ;; Exceeded rounds. Summarize eval results as natural language.
    (when last-eval-result
      (%log :info "sexp-eval" "REPL: final summary from eval data")
      (handler-case
          (backend-complete
           (format nil "~A~%~%Data gathered:~%~A~%~%Answer naturally: ~A"
                   bootstrap
                   (subseq last-eval-result 0 (min 1200 (length last-eval-result)))
                   user-text)
           (or model "auto"))
        (error () nil)))))
