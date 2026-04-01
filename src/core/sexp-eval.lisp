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

;; Read from DNA constraints — DNA defines the hard limits.
(defparameter *repl-max-result-chars*
  (or (ignore-errors (dna-constraint :repl-max-result-chars)) 1500))
(defparameter *repl-max-rounds*
  (or (ignore-errors (dna-constraint :repl-max-rounds)) 5))

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
     (+               (apply #'+ args))
     (-               (apply #'- args))
     (*               (apply #'* args))
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
     ;; ── Workspace tools (Rust actors — the agent's hands) ─────
     (read-file       (apply #'%prim-read-file args))
     (grep            (apply #'%prim-grep args))
     (list-files      (apply #'%prim-list-files args))
     (file-exists     (%prim-file-exists (first args)))
     (file-info       (%prim-file-info (first args)))
     ;; ── Git operations ────────────────────────────────────────
     (git-status      (%prim-git-status))
     (git-log         (%prim-git-log (or (first args) 10)))
     (git-diff        (%prim-git-diff))
     (git-branch      (%prim-git-branch))
     (git-commit      (apply #'%prim-git-commit args))
     (git-push        (%prim-git-push))
     ;; ── Act on the system ────────────────────────────────────
     (store           (apply #'%prim-store args))
     (spawn           (apply #'%prim-spawn args))
     (tool            (apply #'%prim-tool args))
     (observe-route   (apply #'%prim-observe-route args))
     ;; ── Ouroboros (self-healing + evolution) ─────────────────
     (ouroboros-history (%prim-ouroboros-history))
     (ouroboros-crash   (apply #'%prim-ouroboros-crash args))
     (ouroboros-patch   (apply #'%prim-ouroboros-patch args))
     (dream            (%prim-dream))
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
  "Call any IPC component. Full power. Security is enforced in Rust,
not here — vault requires admin-intent signature, policy requires owner auth.
The REPL has full Lisp power; Rust is the boundary."
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
          (let* ((reply (ipc-call "(:component \"memory-field\" :op \"basin-status\")")))
            (if (and reply (stringp reply))
                (let* ((*read-eval* nil)
                       (parsed (ignore-errors (read-from-string reply)))
                       (basin (when (listp parsed) (getf (cdr parsed) :current))))
                  (format nil "Basin: ~A" (or basin "unknown")))
                "(basin unavailable)"))))
      "(basin unavailable)"))

;; ── Workspace primitives (Rust actors — the agent's hands) ──────

(defun %prim-read-file (&rest args)
  (let ((path (first args))
        (offset (or (second args) 0))
        (limit (or (third args) 200)))
    (if (and path (stringp path))
        (or (ignore-errors (workspace-read-file path :offset offset :limit limit))
            "(read-file: not found)")
        "(read-file: path required)")))

(defun %prim-grep (&rest args)
  (let ((pattern (first args))
        (path (or (second args) ".")))
    (if (and pattern (stringp pattern))
        (or (ignore-errors (workspace-grep pattern path))
            "(grep: no results)")
        "(grep: pattern required)")))

(defun %prim-list-files (&rest args)
  (let ((path (or (first args) ".")))
    (or (ignore-errors (workspace-list-files path))
        "(list-files: error)")))

(defun %prim-file-exists (path)
  (if (and path (stringp path))
      (if (ignore-errors (workspace-file-exists-p path)) "exists" "not found")
      "(file-exists: path required)"))

(defun %prim-file-info (path)
  (if (and path (stringp path))
      (or (ignore-errors (workspace-file-info path))
          "(file-info: error)")
      "(file-info: path required)"))

;; ── Git operation primitives ──────────────────────────────────────

(defun %prim-git-status ()
  (or (ignore-errors (git-status)) "(git-status unavailable)"))

(defun %prim-git-log (n)
  (or (ignore-errors (git-log (or n 10))) "(git-log unavailable)"))

(defun %prim-git-diff ()
  (or (ignore-errors (git-diff)) "(git-diff unavailable)"))

(defun %prim-git-branch ()
  (or (ignore-errors (git-branch)) "(git-branch unavailable)"))

(defun %prim-git-commit (&rest args)
  (let ((message (first args)))
    (if (and message (stringp message) (> (length message) 0))
        (or (ignore-errors (git-commit message)) "(git-commit failed)")
        "(git-commit: message required)")))

(defun %prim-git-push ()
  (or (ignore-errors (git-push)) "(git-push failed)"))

;; ── Ouroboros + Dreaming primitives ───────────────────────────────

(defun %prim-ouroboros-history ()
  (or (ignore-errors (ouroboros-history)) "(ouroboros unavailable)"))

(defun %prim-ouroboros-crash (&rest args)
  (let ((comp (or (first args) "unknown"))
        (detail (or (second args) "manual crash record")))
    (if (ignore-errors (ouroboros-record-crash comp detail))
        (format nil "Crash recorded: ~A" comp)
        "(ouroboros-crash failed)")))

(defun %prim-ouroboros-patch (&rest args)
  (let ((comp (or (first args) "unknown"))
        (body (or (second args) "")))
    (if (and (stringp body) (> (length body) 0))
        (if (ignore-errors (ouroboros-write-patch comp body))
            (format nil "Patch written for ~A" comp)
            "(ouroboros-patch failed)")
        "(ouroboros-patch: body required)")))

(defun %prim-dream ()
  (or (ignore-errors
        (when (and (fboundp 'memory-field-port-ready-p)
                   (funcall 'memory-field-port-ready-p))
          (let* ((report (memory-field-dream))
                 (results (when report (%apply-dream-results report))))
            (format nil "Dream: pruned=~D crystallized=~D"
                    (or (getf results :pruned) 0)
                    (or (getf results :crystallized) 0)))))
      "(dream unavailable)"))

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
         (profile (ignore-errors (%profile-by-id model)))
         (cost (if profile (or (getf profile :cost) 5) 5))
         (cost-factor (/ 1.0 (+ 1.0 (float cost)))))
    (+ (* 0.5 fluency) (* 0.3 speed) (* 0.2 cost-factor))))

(defun %select-model-by-repl-perf (prompt)
  "Select the best model by measured REPL performance. Start from free, escalate.
   No hardcoded model names — purely data-driven."
  (let* ((tier-pool (ignore-errors (%tier-model-pool *routing-tier*)))
         (all-pool (or tier-pool
                       (ignore-errors (%tier-model-pool :auto))
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
         ;; ONE bootstrap, always the same.
         (bootstrap (dna-system-prompt))
         ;; Recall from memory field (ONE recall function).
         (recalled (ignore-errors
                     (let ((entries (memory-recall user-text :limit 5)))
                       (when entries
                         (with-output-to-string (out)
                           (dolist (e entries)
                             (let ((text (%entry-text e)))
                               (when (and (stringp text) (> (length text) 10))
                                 (write-string (subseq text 0 (min 200 (length text))) out)
                                 (terpri out)))))))))
         ;; ONE prompt format: bootstrap + context + question.
         (current-prompt
           (format nil "~A~:[~;~%~%CONTEXT:~%~A~]~%~%USER: ~A"
                   bootstrap
                   (and recalled (> (length recalled) 0)) recalled
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
                    (format nil "~A~%~%EVAL_RESULT:~%~A~%~%USER asked: ~A~%Respond naturally via (respond \"...\") or execute more code."
                            bootstrap (or last-eval-result "") user-text)))
              (used-model (or (ignore-errors (%select-model user-text)) ""))
              (call-start (get-internal-real-time)))

          (let ((llm-output
                  (handler-case (backend-complete round-prompt used-model)
                    (error (c)
                      (%log :warn "sexp-eval" "REPL ~D error: ~A" round c)
                      (%record-repl-perf used-model :error)
                      nil)))
                (latency-ms (truncate (* 1000 (/ (- (get-internal-real-time) call-start)
                                                  (float internal-time-units-per-second))))))
            (cond
              ((null llm-output)
               (%log :info "sexp-eval" "REPL ~D: LLM unavailable" round)
               (%record-repl-perf used-model :unavailable :latency-ms latency-ms)
               (when last-eval-result
                 (%log :info "sexp-eval" "REPL: using eval data from previous rounds")
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
                      (more (ignore-errors
                              (memory-semantic-recall-block query :limit 3 :max-chars 800))))
                 (%log :info "sexp-eval" "REPL ~D: RECALL query=[~A]" round
                       (subseq query 0 (min 40 (length query))))
                 (setf last-eval-result (or more "(no additional context found)"))))

              ;; Natural language → return to user.
              (t
               (%record-repl-perf used-model :natural :latency-ms latency-ms)
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
             (or (ignore-errors (%select-model user-text)) ""))
          (error () nil))))))
