;;; admin-intent.lisp — Port: Ed25519 admin intent verification.

(in-package :harmonia)

(defparameter *admin-intent-lib* nil)

(cffi:defcfun ("harmonia_admin_intent_init" %admin-intent-init) :int)
(cffi:defcfun ("harmonia_admin_intent_verify_with_vault" %admin-intent-verify-with-vault) :int
  (action :string)
  (params :string)
  (sig-hex :string)
  (pubkey-symbol :string))
(cffi:defcfun ("harmonia_admin_intent_last_error" %admin-intent-last-error) :pointer)
(cffi:defcfun ("harmonia_admin_intent_free_string" %admin-intent-free-string) :void
  (ptr :pointer))

(defun admin-intent-last-error ()
  (let ((ptr (%admin-intent-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%admin-intent-free-string ptr)))))

(defun init-admin-intent-port ()
  (ensure-cffi)
  (setf *admin-intent-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_admin_intent.dylib")))
  (let ((rc (%admin-intent-init)))
    (runtime-log *runtime* :admin-intent-init (list :status rc))
    (zerop rc)))

(defun admin-intent-verify-with-vault (action params sig-hex &optional (pubkey-symbol "admin-ed25519-pubkey"))
  "Verify signed admin intent against public key stored in vault."
  (let ((rc (%admin-intent-verify-with-vault action params sig-hex pubkey-symbol)))
    (cond
      ((= rc 1) t)
      ((= rc 0) nil)
      (t
       (runtime-log *runtime* :admin-intent-verify-error
                    (list :action action :error (admin-intent-last-error)))
       nil))))
