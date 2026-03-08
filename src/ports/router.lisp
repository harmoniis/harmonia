;;; router.lisp — Port: LLM completion via multi-provider router CFFI.

(in-package :harmonia)

(defparameter *router-lib* nil)

(cffi:defcfun ("harmonia_openrouter_init" %openrouter-init) :int)
(cffi:defcfun ("harmonia_openrouter_complete" %openrouter-complete) :pointer
  (prompt :string)
  (model :string))
(cffi:defcfun ("harmonia_openrouter_last_error" %openrouter-last-error) :pointer)
(cffi:defcfun ("harmonia_openrouter_free_string" %openrouter-free-string) :void
  (ptr :pointer))

(defun init-router-port ()
  (ensure-cffi)
  (setf *router-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_openrouter.dylib")))

  (let ((or-status (%openrouter-init)))
    (runtime-log *runtime* :native-init (list :openrouter or-status))
    (zerop or-status)))

(defun router-healthcheck ()
  "Quick liveness check — returns T if the router lib is loaded."
  (and *router-lib* t))

(defun backend-last-error ()
  (let ((ptr (%openrouter-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%openrouter-free-string ptr)))))

(defun backend-complete (prompt &optional model)
  (let ((ptr (%openrouter-complete prompt (or model ""))))
    (if (cffi:null-pointer-p ptr)
        (error "LLM request failed: ~A" (backend-last-error))
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%openrouter-free-string ptr)))))
