;;; router.lisp — Port: generic LLM provider routing via provider-router CFFI.

(in-package :harmonia)

(defparameter *router-lib* nil)

(cffi:defcfun ("harmonia_provider_router_init" %provider-router-init) :int)
(cffi:defcfun ("harmonia_provider_router_healthcheck" %provider-router-healthcheck) :int)
(cffi:defcfun ("harmonia_provider_router_complete" %provider-router-complete) :pointer
  (prompt :string)
  (model :string))
(cffi:defcfun ("harmonia_provider_router_complete_for_task" %provider-router-complete-for-task) :pointer
  (prompt :string)
  (task-hint :string))
(cffi:defcfun ("harmonia_provider_router_list_models" %provider-router-list-models) :pointer)
(cffi:defcfun ("harmonia_provider_router_select_model" %provider-router-select-model) :pointer
  (task-hint :string))
(cffi:defcfun ("harmonia_provider_router_list_backends" %provider-router-list-backends) :pointer)
(cffi:defcfun ("harmonia_provider_router_backend_status" %provider-router-backend-status) :pointer
  (name :string))
(cffi:defcfun ("harmonia_provider_router_last_error" %provider-router-last-error) :pointer)
(cffi:defcfun ("harmonia_provider_router_free_string" %provider-router-free-string) :void
  (ptr :pointer))

(defun %provider-router-string (thunk)
  (let ((ptr (funcall thunk)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%provider-router-free-string ptr)))))

(defun %parse-router-sexp (text)
  (when (and text (> (length text) 0) (not (string= text "nil")))
    (let ((*read-eval* nil))
      (ignore-errors (read-from-string text)))))

(defun init-router-port ()
  (ensure-cffi)
  (setf *router-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_provider_router.dylib")))
  (let ((status (%provider-router-init)))
    (runtime-log *runtime* :native-init (list :provider-router status))
    (zerop status)))

(defun router-healthcheck ()
  (and *router-lib* (= (%provider-router-healthcheck) 1)))

(defun backend-last-error ()
  (%provider-router-string #'%provider-router-last-error))

(defun backend-complete (prompt &optional model)
  (with-trace ("backend-complete" :kind :llm
               :metadata (list :model (or model "auto")
                               :prompt-length (length (or prompt ""))))
    (let ((ptr (%provider-router-complete prompt (or model ""))))
      (if (cffi:null-pointer-p ptr)
          (error "LLM request failed: ~A" (backend-last-error))
          (unwind-protect
               (cffi:foreign-string-to-lisp ptr)
            (%provider-router-free-string ptr))))))

(defun backend-complete-for-task (prompt task-hint)
  (let ((ptr (%provider-router-complete-for-task prompt (or task-hint ""))))
    (if (cffi:null-pointer-p ptr)
        (error "LLM request failed: ~A" (backend-last-error))
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%provider-router-free-string ptr)))))

(defun backend-list-models ()
  (%provider-router-string #'%provider-router-list-models))

(defun backend-select-model (task-hint)
  (%provider-router-string (lambda () (%provider-router-select-model (or task-hint "")))))

(defun backend-list-backends ()
  (%parse-router-sexp (%provider-router-string #'%provider-router-list-backends)))

(defun backend-backend-status (&optional (name ""))
  (%parse-router-sexp (%provider-router-string
                       (lambda () (%provider-router-backend-status (or name ""))))))
