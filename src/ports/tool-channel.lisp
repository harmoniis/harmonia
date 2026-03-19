;;; tool-channel.lisp — Port: standardised tool channel invocation via gateway IPC.
;;;
;;; Replaces ad-hoc tool FFI with a uniform invoke contract. Tools are
;;; registered and dispatched through the gateway's ToolRegistry, which
;;; applies signal-integrity wrapping and dissonance scoring to all outputs.

(in-package :harmonia)

(defun init-tool-channel-port ()
  "Initialise the tool channel port. Gateway must already be initialised."
  t)

(defun tool-channel-register (name so-path config-sexp &optional (security-label "authenticated"))
  "Register a tool with the gateway's ToolRegistry."
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"register-tool\" :name \"~A\" :so-path \"~A\" :config \"~A\" :security-label \"~A\")"
                        (sexp-escape-lisp name) (sexp-escape-lisp so-path)
                        (sexp-escape-lisp config-sexp) (sexp-escape-lisp security-label)))))
    (when (ipc-reply-error-p reply)
      (error "tool registration failed for ~A: ~A" name reply))
    t))

(defun tool-channel-invoke (name operation params-sexp)
  "Invoke a tool channel operation via the gateway.
   Returns the tool result as an s-expression string.
   Performs harmonic matrix route check before invocation."
  (harmonic-matrix-route-or-error "orchestrator" name)
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"invoke-tool\" :name \"~A\" :operation \"~A\" :params \"~A\")"
                        (sexp-escape-lisp name) (sexp-escape-lisp operation)
                        (sexp-escape-lisp params-sexp)))))
    (if (ipc-reply-error-p reply)
        (progn
          (harmonic-matrix-observe-route "orchestrator" name nil 1)
          (error "tool invoke failed ~A/~A: ~A" name operation reply))
        (progn
          (harmonic-matrix-observe-route "orchestrator" name t 1)
          (or (ipc-extract-value reply) "")))))

(defun tool-channel-list ()
  "List all registered tool names."
  (ipc-extract-value
   (ipc-call "(:component \"gateway\" :op \"list-tools\")")))

(defun tool-channel-capabilities (name)
  "Get the capability list for a registered tool."
  (or (ipc-extract-value
       (ipc-call (format nil "(:component \"gateway\" :op \"tool-capabilities\" :name \"~A\")"
                         (sexp-escape-lisp name))))
      "nil"))

(defun tool-channel-status (name)
  "Get the status of a registered tool."
  (or (ipc-extract-value
       (ipc-call (format nil "(:component \"gateway\" :op \"tool-status\" :name \"~A\")"
                         (sexp-escape-lisp name))))
      "nil"))
