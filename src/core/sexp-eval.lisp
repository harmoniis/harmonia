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
  (or (handler-case (dna-constraint :repl-max-result-chars) (error () nil)) 1500))
(defparameter *repl-max-rounds*
  (or (handler-case (dna-constraint :repl-max-rounds) (error () nil)) 5))

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
  ;; Pipeline trace: every primitive call with args preview
  (%pipeline-trace :sexp-primitive-call
    :op op
    :args-count (length args)
    :args-preview (%clip-prompt (format nil "~S" args) 120))
  (%bound-result
   (case op
     ;; ── Self-discovery ────────────────────────────────────────
     (env             (%prim-env))
     ;; ── Read the system (L1 field → L2 chronicle → L3 palace) ──
     (field           (%prim-field))
     (recall          (apply #'%prim-recall args))
     (ipc             (apply #'%prim-ipc args))
     (introspect      (%prim-introspect))
     (status          (%prim-status))
     (chaos-risk      (%prim-chaos-risk))
     (basin           (%prim-basin))
     (models          (%prim-models))
     (route-check     (apply #'%prim-route-check args))
     ;; ── Compose (common names models expect) ───────────────
     (format          (apply #'format nil args))
     (str             (apply #'concatenate 'string (mapcar #'princ-to-string args)))
     (cat             (apply #'concatenate 'string (mapcar #'princ-to-string args)))
     (concat          (apply #'concatenate 'string (mapcar #'princ-to-string args)))
     (join            (format nil "~{~A~^ ~}" (first args)))
     (getf            (getf (first args) (second args)))
     (length          (length (first args)))
     (subseq          (apply #'subseq args))
     (concatenate     (apply #'concatenate 'string args))
     (string-downcase (string-downcase (first args)))
     (string-upcase   (string-upcase (first args)))
     (to-string       (princ-to-string (first args)))
     (+               (apply #'+ args))
     (-               (apply #'- args))
     (*               (apply #'* args))
     (/               (/ (first args) (second args)))
     (>               (> (first args) (second args)))
     (<               (< (first args) (second args)))
     (=               (= (first args) (second args)))
     (not             (not (first args)))
     (and             (every #'identity args))
     (or              (some #'identity args))
     (list            args)
     ;; ── List access (models always try these) ──────────────
     (car             (car (first args)))
     (cdr             (cdr (first args)))
     (cadr            (cadr (first args)))
     (caddr           (caddr (first args)))
     (nth             (nth (first args) (second args)))
     (first           (first (first args)))
     (second          (second (first args)))
     (third           (third (first args)))
     (fourth          (fourth (first args)))
     (rest            (rest (first args)))
     (last            (car (last (first args))))
     (cons            (cons (first args) (second args)))
     (append          (append (first args) (second args)))
     (mapcar          (mapcar (first args) (second args)))
     (remove-if       (remove-if (first args) (second args)))
     (assoc           (assoc (first args) (second args)))
     (princ-to-string (princ-to-string (first args)))
     ;; ── Workspace tools (Rust actors — the agent's hands) ─────
     (read-file       (apply #'%prim-read-file args))
     (grep            (apply #'%prim-grep args))
     (list-files      (apply #'%prim-list-files args))
     (file-exists     (%prim-file-exists (first args)))
     (file-info       (%prim-file-info (first args)))
     (write-file      (apply #'%prim-write-file args))
     (append-file     (apply #'%prim-append-file args))
     (exec            (apply #'%prim-exec args))
     ;; ── Act on the system ────────────────────────────────────
     (store           (apply #'%prim-store args))
     (spawn           (apply #'%prim-spawn args))
     (tool            (apply #'%prim-tool args))
     (observe-route   (apply #'%prim-observe-route args))
     ;; ── Memory field maintenance (heartbeat decides when) ─────
     (dream            (%prim-dream))
     (meditate         (%prim-meditate))
     ;; ── Evolution (vitruvian-gated) ──────────────────────────
     (evolve          (apply #'%prim-evolve args))
     (rewrite-plan    (%prim-rewrite-plan))
     ;; ── MemPalace (graph-structured knowledge) ──────────────────
     (palace-search   (apply #'%prim-palace-search args))
     (palace-file     (apply #'%prim-palace-file args))
     (palace-graph    (apply #'%prim-palace-graph args))
     (palace-compress (apply #'%prim-palace-compress args))
     (palace-context  (apply #'%prim-palace-context args))
     (palace-kg       (apply #'%prim-palace-kg args))
     ;; ── Terraphon (platform datamining tools) ─────────────────────
     (datamine        (apply #'%prim-datamine args))
     (datamine-remote (apply #'%prim-datamine-remote args))
     (datamine-for    (apply #'%prim-datamine-for args))
     (lodes           (%prim-lodes))
     ;; ── Web + Python (datamining and document processing) ──────
     (fetch-url       (%prim-fetch-url (first args)))
     (fetch           (%prim-fetch-url (first args)))
     (browse          (apply #'%prim-browse args))
     (python          (%prim-python (first args)))
     (py              (%prim-python (first args)))
     (search-web      (%prim-search-web (first args)))
     (search          (%prim-search-web (first args)))
     (convert-doc     (%prim-convert-doc (first args)))
     (convert         (%prim-convert-doc (first args)))
     (markitdown      (%prim-convert-doc (first args)))
     ;; ── Respond fallback (should be caught in %reval special forms) ──
     (respond         (throw 'repl-respond (first args)))
     ;; ── Unknown ──────────────────────────────────────────────
     (otherwise       (format nil "(:error \"unknown primitive: ~A\")" op)))))

(defun %bound-result (val)
  (let ((s (if (stringp val) val (princ-to-string val))))
    (if (> (length s) *repl-max-result-chars*)
        (concatenate 'string (subseq s 0 *repl-max-result-chars*) "...[truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVE TABLE — the function whitelist registry
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *repl-primitives*
  '(;; system
    env field recall respond store read-file grep list-files exec write-file
    append-file file-exists file-info status introspect basin models
    chaos-risk dream meditate spawn evolve ipc route-check tool
    ;; palace + datamining + web + python
    palace-search palace-file palace-graph palace-compress palace-context palace-kg
    datamine datamine-remote datamine-for lodes
    fetch-url fetch browse python py search-web search convert-doc convert markitdown
    ;; compose
    format str cat concat join getf length subseq concatenate
    string-downcase string-upcase to-string princ-to-string
    + - * / > < = not and or
    ;; list ops
    list car cdr cadr caddr nth first second third fourth
    rest last cons append mapcar remove-if assoc
    ;; special forms
    let if when unless progn quote)
  "The primitive table — derived, not described. (env) returns this.")
