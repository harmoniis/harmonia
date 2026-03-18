;;; tool-channel.lisp — Port: standardised tool channel invocation via gateway.
;;;
;;; Replaces ad-hoc tool FFI with a uniform invoke contract. Tools are
;;; registered and dispatched through the gateway's ToolRegistry, which
;;; applies signal-integrity wrapping and dissonance scoring to all outputs.

(in-package :harmonia)

;; Gateway tool FFI bindings
(cffi:defcfun ("harmonia_gateway_register_tool" %gateway-register-tool) :int
  (name :string) (so-path :string) (config-sexp :string) (security-label :string))
(cffi:defcfun ("harmonia_gateway_unregister_tool" %gateway-unregister-tool) :int
  (name :string))
(cffi:defcfun ("harmonia_gateway_invoke_tool" %gateway-invoke-tool) :pointer
  (name :string) (operation :string) (params-sexp :string))
(cffi:defcfun ("harmonia_gateway_list_tools" %gateway-list-tools) :pointer)
(cffi:defcfun ("harmonia_gateway_tool_capabilities" %gateway-tool-caps) :pointer
  (name :string))
(cffi:defcfun ("harmonia_gateway_tool_status" %gateway-tool-status) :pointer
  (name :string))
(cffi:defcfun ("harmonia_gateway_reload_tool" %gateway-reload-tool) :int
  (name :string))

(defun init-tool-channel-port ()
  "Initialise the tool channel port. Gateway must already be initialised."
  t)

(defun tool-channel-register (name so-path config-sexp &optional (security-label "authenticated"))
  "Register a tool with the gateway's ToolRegistry."
  (let ((rc (%gateway-register-tool name so-path config-sexp security-label)))
    (unless (zerop rc)
      (error "tool registration failed for ~A: ~A" name
             (%last-error-string #'harmonia-gateway-last-error #'harmonia-gateway-free-string)))
    t))

(defun tool-channel-invoke (name operation params-sexp)
  "Invoke a tool channel operation via the gateway.
   Returns the tool result as an s-expression string.
   Performs harmonic matrix route check before invocation."
  (harmonic-matrix-route-or-error "orchestrator" name)
  (let ((ptr (%gateway-invoke-tool name operation params-sexp)))
    (if (cffi:null-pointer-p ptr)
        (let ((err (%last-error-string #'harmonia-gateway-last-error #'harmonia-gateway-free-string)))
          (harmonic-matrix-observe-route "orchestrator" name nil 1)
          (error "tool invoke failed ~A/~A: ~A" name operation err))
        (let ((result (unwind-protect
                           (cffi:foreign-string-to-lisp ptr)
                        (harmonia-gateway-free-string ptr))))
          (harmonic-matrix-observe-route "orchestrator" name t 1)
          result))))

(defun tool-channel-list ()
  "List all registered tool names."
  (let ((ptr (%gateway-list-tools)))
    (if (cffi:null-pointer-p ptr)
        nil
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (harmonia-gateway-free-string ptr)))))

(defun tool-channel-capabilities (name)
  "Get the capability list for a registered tool."
  (let ((ptr (%gateway-tool-caps name)))
    (if (cffi:null-pointer-p ptr)
        "nil"
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (harmonia-gateway-free-string ptr)))))

(defun tool-channel-status (name)
  "Get the status of a registered tool."
  (let ((ptr (%gateway-tool-status name)))
    (if (cffi:null-pointer-p ptr)
        "nil"
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (harmonia-gateway-free-string ptr)))))
