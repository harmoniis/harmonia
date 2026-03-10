;;; store.lisp — Port: non-secret runtime configuration key-values via config-store CFFI.

(in-package :harmonia)

(defparameter *config-store-lib* nil)

;; ─── Simple FFI bindings (no policy enforcement) ─────────────────────

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

;; ─── Component-aware FFI bindings (policy-gated) ─────────────────────

(cffi:defcfun ("harmonia_config_store_get_for" %config-get-for) :pointer
  (component :string)
  (scope :string)
  (key :string))
(cffi:defcfun ("harmonia_config_store_get_or" %config-get-or) :pointer
  (component :string)
  (scope :string)
  (key :string)
  (default-value :string))
(cffi:defcfun ("harmonia_config_store_set_for" %config-set-for) :int
  (component :string)
  (scope :string)
  (key :string)
  (value :string))
(cffi:defcfun ("harmonia_config_store_delete_for" %config-delete-for) :int
  (component :string)
  (scope :string)
  (key :string))
(cffi:defcfun ("harmonia_config_store_dump" %config-dump) :pointer
  (component :string)
  (scope :string))
(cffi:defcfun ("harmonia_config_store_ingest_env" %config-ingest-env) :int)

;; ─── Init ────────────────────────────────────────────────────────────

(defun init-store-port ()
  (setf *config-store-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_config_store.dylib")))
  (let ((rc (%config-init)))
    (runtime-log *runtime* :config-store-init (list :status rc))
    (zerop rc)))

;; ─── Error helper ────────────────────────────────────────────────────

(defun config-last-error ()
  (let ((ptr (%config-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%config-free-string ptr)))))

;; ─── Simple wrappers (admin-level, no policy check) ──────────────────

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

(defun config-list (&optional (scope ""))
  (let ((ptr (%config-list (or scope ""))))
    (if (cffi:null-pointer-p ptr)
        '()
        (unwind-protect
             (let ((raw (cffi:foreign-string-to-lisp ptr)))
               (if (zerop (length raw))
                   '()
                   (%split-lines raw)))
          (%config-free-string ptr)))))

;; ─── Component-aware wrappers (policy-gated) ─────────────────────────

(defun config-get-for (component key &optional (scope component))
  "Get a config value with policy enforcement and env fallback chain."
  (let ((ptr (%config-get-for component scope key)))
    (if (cffi:null-pointer-p ptr)
        nil
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%config-free-string ptr)))))

(defun config-get-or (component key default &optional (scope component))
  "Get a config value with default, policy enforcement and env fallback chain."
  (let ((ptr (%config-get-or component scope key (or default ""))))
    (if (cffi:null-pointer-p ptr)
        (or default "")
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%config-free-string ptr)))))

(defun config-set-for (component key value &optional (scope component))
  "Set a config value with policy enforcement."
  (let ((rc (%config-set-for component scope key (or value ""))))
    (unless (zerop rc)
      (error "Config store set-for failed: ~A" (config-last-error)))
    t))

(defun config-delete-for (component key &optional (scope component))
  "Delete a config value (admin-only via policy)."
  (let ((rc (%config-delete-for component scope key)))
    (unless (zerop rc)
      (error "Config store delete-for failed: ~A" (config-last-error)))
    t))

(defun config-dump (component &optional (scope component))
  "Dump all key=value pairs in a scope as a list of lines."
  (let ((ptr (%config-dump component scope)))
    (if (cffi:null-pointer-p ptr)
        '()
        (unwind-protect
             (let ((raw (cffi:foreign-string-to-lisp ptr)))
               (if (zerop (length raw))
                   '()
                   (%split-lines raw)))
          (%config-free-string ptr)))))

(defun config-ingest-env ()
  "Seed config DB from environment variables (first-run only)."
  (let ((rc (%config-ingest-env)))
    (unless (zerop rc)
      (error "Config store ingest-env failed: ~A" (config-last-error)))
    t))
