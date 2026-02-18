;;; config-store.lisp — CFFI bridge for runtime non-secret configuration key-values.

(in-package :harmonia)

(defparameter *config-store-lib* nil)

(cffi:defcfun ("harmonia_config_store_init" %config-init) :int)
(cffi:defcfun ("harmonia_config_store_set" %config-set) :int
  (scope :string)
  (key :string)
  (value :string))
(cffi:defcfun ("harmonia_config_store_get" %config-get) :pointer
  (scope :string)
  (key :string))
(cffi:defcfun ("harmonia_config_store_list" %config-list) :pointer
  (scope :string))
(cffi:defcfun ("harmonia_config_store_last_error" %config-last-error) :pointer)
(cffi:defcfun ("harmonia_config_store_free_string" %config-free-string) :void
  (ptr :pointer))

(defun %config-release-lib-path (name)
  (merge-pathnames
   name
   (merge-pathnames "../../target/release/" *boot-file*)))

(defun init-config-store-backend ()
  (setf *config-store-lib*
        (cffi:load-foreign-library (%config-release-lib-path "libharmonia_config_store.dylib")))
  (let ((rc (%config-init)))
    (runtime-log *runtime* :config-store-init (list :status rc))
    (zerop rc)))

(defun config-last-error ()
  (let ((ptr (%config-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%config-free-string ptr)))))

(defun config-set (key value &optional (scope "global"))
  (let ((rc (%config-set scope key (or value ""))))
    (unless (zerop rc)
      (error "Config store set failed: ~A" (config-last-error)))
    t))

(defun config-get (key &optional (scope "global"))
  (let ((ptr (%config-get scope key)))
    (if (cffi:null-pointer-p ptr)
        nil
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%config-free-string ptr)))))

(defun %config-split-lines (text)
  (let ((parts '())
        (start 0))
    (loop for i = (position #\Newline text :start start)
          do (push (subseq text start (or i (length text))) parts)
          if (null i) do (return)
          do (setf start (1+ i)))
    (remove-if #'(lambda (s) (zerop (length s))) (nreverse parts))))

(defun config-list (&optional (scope ""))
  (let ((ptr (%config-list (or scope ""))))
    (if (cffi:null-pointer-p ptr)
        '()
        (unwind-protect
             (let ((raw (cffi:foreign-string-to-lisp ptr)))
               (if (zerop (length raw))
                   '()
                   (%config-split-lines raw)))
          (%config-free-string ptr)))))
