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
;;; PRIMITIVE PROTOCOL — declarative registration via defprimitive macro
;;; ═══════════════════════════════════════════════════════════════════════

(defstruct repl-primitive
  "Protocol entry for a REPL primitive: name, args-spec, doc, handler."
  name       ; symbol — the primitive name
  args-spec  ; string — e.g. "(query &key limit)" or "()"
  doc        ; string — documentation
  handler)   ; function (args env) -> result

(defparameter *primitive-dispatch* (make-hash-table :test 'eq)
  "Registry of REPL primitives. Symbol -> repl-primitive struct.
   Populated by defprimitive calls at load time.")

(defmacro defprimitive (name args-spec doc &body handler-body)
  "Register a REPL primitive into *primitive-dispatch*.
   HANDLER-BODY receives ARGS (evaluated arg list) and ENV (lexical env)."
  `(setf (gethash ',name *primitive-dispatch*)
         (make-repl-primitive
           :name ',name
           :args-spec ,args-spec
           :doc ,doc
           :handler (lambda (args env)
                      (declare (ignorable args env))
                      ,@handler-body))))

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
  "Dispatch a primitive call via *primitive-dispatch* hash table.
   Only registered primitives are callable. Data-driven, not case-driven."
  ;; Pipeline trace: every primitive call with args preview
  (%pipeline-trace :sexp-primitive-call
    :op op
    :args-count (length args)
    :args-preview (%clip-prompt (format nil "~S" args) 120))
  ;; NO %bound-result here — raw values flow through let bindings intact.
  ;; Truncation happens ONLY at display boundaries (eval-all-forms, respond).
  (let ((prim (gethash op *primitive-dispatch*)))
    (if prim
        (funcall (repl-primitive-handler prim) args env)
        (format nil "(:error \"unknown primitive: ~A\")" op))))

(defun %bound-result (val)
  (let ((s (if (stringp val) val (princ-to-string val))))
    (if (> (length s) *repl-max-result-chars*)
        (concatenate 'string (subseq s 0 *repl-max-result-chars*) "...[truncated]")
        s)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; PRIMITIVE TABLE — the function whitelist registry
;;; ═══════════════════════════════════════════════════════════════════════

(defun %compute-repl-primitives ()
  "Derive the primitive list from *primitive-dispatch*. Data, not description."
  (let ((names '()))
    (maphash (lambda (k v) (declare (ignore v)) (push k names))
             *primitive-dispatch*)
    (append (sort names #'string< :key #'symbol-name)
            ;; Special forms are handled in %reval, not in the dispatch table
            '(let if when unless progn quote respond))))

;; Initialized as nil; set after all defprimitive forms have loaded (end of repl-primitives.lisp).
(defparameter *repl-primitives* nil
  "The primitive table — computed from *primitive-dispatch*. (env) returns this.")
