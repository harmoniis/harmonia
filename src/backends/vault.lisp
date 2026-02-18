;;; vault.lisp — CFFI bridge for write-only vault operations from Lisp.

(in-package :harmonia)

(defparameter *vault-lib* nil)

(cffi:defcfun ("harmonia_vault_init" %vault-init) :int)
(cffi:defcfun ("harmonia_vault_set_secret" %vault-set-secret) :int
  (symbol :string)
  (value :string))
(cffi:defcfun ("harmonia_vault_last_error" %vault-last-error) :pointer)
(cffi:defcfun ("harmonia_vault_free_string" %vault-free-string) :void
  (ptr :pointer))

(defun %vault-release-lib-path (name)
  (merge-pathnames
   name
   (merge-pathnames "../../target/release/" *boot-file*)))

(defun init-vault-backend ()
  (setf *vault-lib*
        (cffi:load-foreign-library (%vault-release-lib-path "libharmonia_vault.dylib")))
  (let ((rc (%vault-init)))
    (runtime-log *runtime* :vault-init (list :status rc))
    (zerop rc)))

(defun vault-last-error ()
  (let ((ptr (%vault-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%vault-free-string ptr)))))

(defun vault-set-secret (symbol value)
  (let ((rc (%vault-set-secret symbol value)))
    (unless (zerop rc)
      (error "Vault set failed: ~A" (vault-last-error)))
    t))
