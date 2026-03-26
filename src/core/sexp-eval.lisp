;;; sexp-eval.lisp — Restricted Lisp dialect for the Harmonic REPL.
;;;
;;; The LLM drives the entire system via s-expressions. This is not
;;; 3 hardcoded verbs — it's a sandboxed Lisp where the LLM can compose,
;;; chain, decide, spawn subagents, and evolve the system.
;;;
;;; Code is data. Data is code. Memory field is both.
;;;
;;; The evaluator enforces safety structurally:
;;;   - Lexical sandbox: let creates locals, no global mutation
;;;   - Function whitelist: only registered primitives callable
;;;   - Result bounding: max chars per eval
;;;   - Policy gate: spawn/tool go through existing security
;;;   - Audit trail: every eval logged to Chronicle
;;;
;;; Denied: eval, load, compile, funcall, apply, defun, setf on globals,
;;;         vault-set, vault-delete, policy mutation, data destruction.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONSTANTS
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *repl-max-result-chars* 1500)
(defparameter *repl-max-rounds* 5)

;;; ═══════════════════════════════════════════════════════════════════════
;;; RESTRICTED EVALUATOR — sandboxed Lisp interpreter
;;; ═══════════════════════════════════════════════════════════════════════

(defun %reval (form env)
  "Evaluate FORM in restricted environment ENV. Recursive interpreter.
ENV is an alist of (symbol . value) bindings. No global mutation."
  (cond
    ;; Atoms
    ((null form) nil)
    ((eq form t) t)
    ((numberp form) form)
    ((stringp form) form)
    ((keywordp form) form)
    ;; Variable lookup
    ((symbolp form)
     (let ((binding (assoc form env)))
       (if binding
           (cdr binding)
           (error "Unbound: ~A" form))))
    ;; Lists = function calls or special forms
    ((listp form)
     (let ((op (car form)))
       (case op
         ;; ── Special forms ──────────────────────────────────────
         (quote   (second form))
         (let     (%reval-let (second form) (cddr form) env))
         (if      (%reval-if (second form) (third form) (fourth form) env))
         (when    (when (%reval (second form) env)
                    (%reval-progn (cddr form) env)))
         (unless  (unless (%reval (second form) env)
                    (%reval-progn (cddr form) env)))
         (progn   (%reval-progn (cdr form) env))
         ;; ── Respond: final answer to user (terminates REPL) ────
         (respond (throw 'repl-respond (%reval (second form) env)))
         ;; ── Primitives (evaluated args) ────────────────────────
         (t       (%reval-call op (mapcar (lambda (a) (%reval a env))
                                          (cdr form))
                               env)))))
    (t (error "Cannot evaluate: ~S" form))))

(defun %reval-let (bindings body env)
  "Evaluate let bindings, extend env, evaluate body."
  (let ((new-env env))
    (dolist (binding bindings)
      (let* ((var (if (listp binding) (car binding) binding))
             (val (if (listp binding) (%reval (second binding) env) nil)))
        (push (cons var val) new-env)))
    (%reval-progn body new-env)))

(defun %reval-if (test then else env)
  "Conditional evaluation."
  (if (%reval test env)
      (%reval then env)
      (when else (%reval else env))))

(defun %reval-progn (forms env)
  "Evaluate forms in sequence, return last."
  (let ((result nil))
    (dolist (form forms result)
      (setf result (%reval form env)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVE DISPATCH — the function whitelist
;;; ═══════════════════════════════════════════════════════════════════════

(defun %reval-call (op args env)
  "Dispatch a primitive call. Only whitelisted functions."
  (declare (ignore env))
  (%bound-result
   (case op
     ;; ── Read the system ──────────────────────────────────────
     (recall          (apply #'%prim-recall args))
     (ipc             (apply #'%prim-ipc args))
     (introspect      (%prim-introspect))
     (chaos-risk      (%prim-chaos-risk))
     (basin           (%prim-basin))
     (models          (%prim-models))
     (route-check     (apply #'%prim-route-check args))
     ;; ── Compose ──────────────────────────────────────────────
     (format          (apply #'format nil args))
     (getf            (getf (first args) (second args)))
     (length          (length (first args)))
     (subseq          (apply #'subseq args))
     (concatenate     (apply #'concatenate 'string args))
     (string-downcase (string-downcase (first args)))
     (>               (> (first args) (second args)))
     (<               (< (first args) (second args)))
     (=               (= (first args) (second args)))
     (not             (not (first args)))
     (and             (every #'identity args))
     (or              (some #'identity args))
     (list            args)
     (first           (first (first args)))
     (second          (second (first args)))
     (third           (third (first args)))
     (princ-to-string (princ-to-string (first args)))
     ;; ── Act on the system ────────────────────────────────────
     (store           (apply #'%prim-store args))
     (spawn           (apply #'%prim-spawn args))
     (tool            (apply #'%prim-tool args))
     (observe-route   (apply #'%prim-observe-route args))
     ;; ── Evolution (vitruvian-gated) ──────────────────────────
     (evolve          (apply #'%prim-evolve args))
     (rewrite-plan    (%prim-rewrite-plan))
     ;; ── Unknown ──────────────────────────────────────────────
     (otherwise       (format nil "(:error \"unknown primitive: ~A\")" op)))))

(defun %bound-result (val)
  (let ((s (if (stringp val) val (princ-to-string val))))
    (if (> (length s) *repl-max-result-chars*)
        (concatenate 'string (subseq s 0 *repl-max-result-chars*) "...[truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVES — system interface
;;; ═══════════════════════════════════════════════════════════════════════

;; ── recall: smart memory search ─────────────────────────────────────

(defun %prim-recall (query &rest kwargs &key (limit 5) (max-chars 1200) verbatim tags since)
  "Smart recall: field resonance, verbatim exact match, tag/time filtering."
  (declare (ignore kwargs))
  (let ((q (if (stringp query) query (princ-to-string query))))
    (cond
      (verbatim
       (or (ignore-errors
             (when (fboundp 'memory-recall-verbatim)
               (let ((entries (funcall 'memory-recall-verbatim q)))
                 (if entries
                     (format nil "~{~A~%~}"
                             (mapcar (lambda (e) (%entry-text e)) entries))
                     "(no verbatim match)"))))
           "(verbatim unavailable)"))
      (t
       (or (ignore-errors
             (let ((r (memory-semantic-recall-block q :limit limit :max-chars max-chars)))
               (if (and r (> (length r) 0)) r "(no memories found)")))
           "(recall unavailable)")))))

;; ── ipc: generic system query ───────────────────────────────────────

(defun %prim-ipc (component op &rest kwargs)
  "Call any IPC component. Fully generic."
  (let* ((comp (if (stringp component) component (princ-to-string component)))
         (operation (if (stringp op) op (princ-to-string op)))
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
                       (sexp-escape-lisp comp) (sexp-escape-lisp operation) kv-str)))
    (or (ipc-call sexp) "(ipc: no response)")))

;; ── System info primitives ──────────────────────────────────────────

(defun %prim-introspect ()
  (or (ignore-errors
        (when (fboundp '%runtime-identity)
          (funcall '%runtime-identity)))
      "(introspect unavailable)"))

(defun %prim-chaos-risk ()
  (or (ignore-errors
        (let ((ctx (runtime-state-harmonic-context *runtime*)))
          (when ctx
            (let ((logistic (getf ctx :logistic)))
              (when logistic (getf logistic :chaos-risk))))))
      0.5))

(defun %prim-basin ()
  (or (ignore-errors
        (when (and (fboundp 'memory-field-port-ready-p)
                   (funcall 'memory-field-port-ready-p))
          (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))
      "(basin unavailable)"))

(defun %prim-models ()
  (or (ignore-errors (ipc-call "(:component \"provider-router\" :op \"list-backends\")"))
      "(models unavailable)"))

(defun %prim-route-check (from to)
  (or (ignore-errors
        (ipc-call (format nil "(:component \"harmonic-matrix\" :op \"route-allowed\" :from \"~A\" :to \"~A\" :signal 0.7 :noise 0.3)"
                          (sexp-escape-lisp from) (sexp-escape-lisp to))))
      "(route check unavailable)"))

;; ── Action primitives ───────────────────────────────────────────────

(defun %prim-store (content &rest kwargs &key tags)
  (declare (ignore kwargs))
  (let ((text (if (stringp content) content (princ-to-string content))))
    (ignore-errors (memory-put :daily text :tags (or tags '(:user-stored))))
    "(:ok stored)"))

(defun %prim-spawn (model &rest kwargs &key task workdir)
  "Spawn a CLI subagent. Non-blocking. Returns actor-id or :deferred."
  (declare (ignore kwargs))
  (let ((m (if (stringp model) model (princ-to-string model)))
        (t-text (or task ""))
        (wd (or workdir "")))
    (or (ignore-errors
          (when (fboundp 'tmux-spawn)
            (let ((actor-id (funcall 'tmux-spawn m wd t-text)))
              (if (and actor-id (>= actor-id 0))
                  (format nil "(:spawned :actor-id ~D :model \"~A\")" actor-id m)
                  "(:error \"spawn failed\")"))))
        "(:error \"spawn unavailable\")")))

(defun %prim-tool (name &rest kwargs)
  "Execute any registered tool."
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
        (format nil "(:error \"tool ~A failed\")" tool-name))))

(defun %prim-observe-route (from to &rest kwargs &key success latency-ms)
  (declare (ignore kwargs))
  (ignore-errors
    (ipc-call (format nil "(:component \"harmonic-matrix\" :op \"observe-route\" :from \"~A\" :to \"~A\" :success ~A :latency-ms ~D)"
                      (sexp-escape-lisp from) (sexp-escape-lisp to)
                      (if success "t" "nil") (or latency-ms 0))))
  "(:ok observed)")

;; ── Evolution (vitruvian-gated) ─────────────────────────────────────

(defun %prim-evolve (&rest kwargs &key reason target)
  (declare (ignore kwargs))
  "Evolution request. Requires vitruvian readiness."
  (let ((ready (ignore-errors
                 (when (fboundp '%harmonic-plan-ready-p)
                   (funcall '%harmonic-plan-ready-p)))))
    (if ready
        (progn
          (ignore-errors
            (memory-put :daily
                        (format nil "Evolution requested: reason=~A target=~A" reason target)
                        :tags '(:evolution :request)))
          (format nil "(:ok :evolution-requested :reason \"~A\" :target \"~A\")" reason target))
        "(:denied \"vitruvian readiness not met — chaos too high or signal too low\")")))

(defun %prim-rewrite-plan ()
  (let ((ctx (ignore-errors (runtime-state-harmonic-context *runtime*))))
    (if ctx
        (let ((plan (getf ctx :plan)))
          (if plan
              (format nil "(:rewrite-plan :ready ~A :signal ~A :noise ~A)"
                      (getf plan :ready)
                      (and (getf plan :vitruvian) (getf (getf plan :vitruvian) :signal))
                      (and (getf plan :vitruvian) (getf (getf plan :vitruvian) :noise)))
              "(no plan computed yet)"))
        "(harmonic context unavailable)")))

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

(defun %eval-all-forms (text)
  "Parse text as restricted Lisp forms and evaluate each. Return combined results."
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
;;; MODEL SELECTION
;;; ═══════════════════════════════════════════════════════════════════════

(defun %repl-select-model (user-text)
  "Complexity-driven model selection."
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
;;; THE HARMONIC REPL — the orchestration core
;;; ═══════════════════════════════════════════════════════════════════════

(defun %orchestrate-repl (prompt &key (max-rounds *repl-max-rounds*))
  "The Harmonic REPL. LLM drives the system via restricted Lisp.
Parallel context pre-gathered. Complexity selects model. No timeouts.
(respond ...) in the dialect terminates the loop and returns to user."
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

    ;; The (respond ...) primitive throws 'repl-respond to exit the loop.
    (catch 'repl-respond
      (loop while (< round max-rounds) do
        (incf round)
        (let ((round-prompt
                (if (= round 1)
                    current-prompt
                    (format nil "~A~%~%EVAL_RESULT:~%~A~%~%USER asked: ~A~%Respond naturally via (respond \"...\") or execute more code."
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

              ;; Output starts with ( → it's code. Evaluate via restricted interpreter.
              ((%is-sexp-output-p llm-output)
               (%log :info "sexp-eval" "REPL ~D: evaluating restricted Lisp" round)
               (setf last-eval-result (%eval-all-forms llm-output)))

              ;; Natural language → return to user.
              (t
               (%log :info "sexp-eval" "REPL ~D: response (~D chars)" round (length llm-output))
               (return-from %orchestrate-repl llm-output))))))

      ;; Exceeded rounds. Final summary.
      (when last-eval-result
        (%log :info "sexp-eval" "REPL: final summary from eval data")
        (handler-case
            (backend-complete
             (format nil "~A~%~%Data gathered:~%~A~%~%Answer the user naturally: ~A"
                     bootstrap
                     (subseq last-eval-result 0 (min 1200 (length last-eval-result)))
                     user-text)
             (or model "auto"))
          (error () nil))))))
