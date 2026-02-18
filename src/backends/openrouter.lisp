;;; openrouter.lisp — CFFI bridge for harmonia_openrouter dylib.

(in-package :harmonia)

(defparameter *cffi-ready* nil)
(defparameter *openrouter-lib* nil)

(cffi:defcfun ("harmonia_openrouter_init" %openrouter-init) :int)
(cffi:defcfun ("harmonia_openrouter_complete" %openrouter-complete) :pointer
  (prompt :string)
  (model :string))
(cffi:defcfun ("harmonia_openrouter_last_error" %openrouter-last-error) :pointer)
(cffi:defcfun ("harmonia_openrouter_free_string" %openrouter-free-string) :void
  (ptr :pointer))

(defun ensure-cffi ()
  (unless *cffi-ready*
    (load #P"~/quicklisp/setup.lisp")
    (let* ((ql-package (find-package :ql))
           (quickload (and ql-package (find-symbol "QUICKLOAD" ql-package))))
      (unless quickload
        (error "Quicklisp did not provide QL:QUICKLOAD"))
      (funcall quickload :cffi))
    (setf *cffi-ready* t)))

(defun %release-lib-path (name)
  (merge-pathnames
   name
   (merge-pathnames "../../target/release/" *boot-file*)))

(defun init-native-backends ()
  (ensure-cffi)
  (setf *openrouter-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_openrouter.dylib")))

  (let ((or-status (%openrouter-init)))
    (runtime-log *runtime* :native-init (list :openrouter or-status))
    (zerop or-status)))

(defun backend-last-error ()
  (let ((ptr (%openrouter-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%openrouter-free-string ptr)))))

(defun backend-complete (prompt &optional (model "qwen/qwen3-coder:free"))
  (let ((ptr (%openrouter-complete prompt model)))
    (if (cffi:null-pointer-p ptr)
        (error "OpenRouter request failed: ~A" (backend-last-error))
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%openrouter-free-string ptr)))))
