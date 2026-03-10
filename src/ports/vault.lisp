;;; vault.lisp — Port: secret key-value storage via harmonia-vault CFFI.
;;; Also provides shared CFFI infrastructure (ensure-cffi, %release-lib-path, %split-lines).

(in-package :harmonia)

;;; --- Shared CFFI infrastructure (loaded first, used by all ports) ---

(defparameter *cffi-ready* nil)

(defun ensure-cffi ()
  (unless *cffi-ready*
    (load #P"~/quicklisp/setup.lisp")
    (let* ((ql-package (find-package :ql))
           (quickload (and ql-package (find-symbol "QUICKLOAD" ql-package))))
      (unless quickload
        (error "Quicklisp did not provide QL:QUICKLOAD"))
      (funcall quickload :cffi))
    (setf *cffi-ready* t)))

(defun %shared-lib-extension ()
  #+darwin "dylib"
  #+windows "dll"
  #-(or darwin windows) "so")

(defun %normalize-lib-name (name)
  (let* ((dot (position #\. name :from-end t))
         (base (if dot (subseq name 0 dot) name))
         (ext (if dot (string-downcase (subseq name (1+ dot))) "")))
    (if (member ext '("dylib" "so" "dll") :test #'string=)
        (concatenate 'string base "." (%shared-lib-extension))
        (concatenate 'string name "." (%shared-lib-extension)))))

(defun %release-lib-roots ()
  (let ((roots '())
        (env (sb-ext:posix-getenv "HARMONIA_LIB_DIR")))
    (when (and env (> (length env) 0))
      (push (pathname (if (char= (char env (1- (length env))) #\/)
                          env
                          (concatenate 'string env "/")))
            roots))
    ;; Dev fallback: target/release/ relative to boot.lisp
    (push (merge-pathnames "../../target/release/" *boot-file*) roots)
    (let* ((home (sb-ext:posix-getenv "HOME"))
           (platform-lib (when home
                           (pathname (concatenate 'string home "/.local/lib/harmonia/")))))
      (when platform-lib
        (push platform-lib roots)))
    (nreverse roots)))

(defun %release-lib-path (name)
  (let* ((normalized (%normalize-lib-name name))
         (candidates (mapcar (lambda (root) (merge-pathnames normalized root))
                             (%release-lib-roots))))
    (or (find-if #'probe-file candidates)
        (first candidates))))

(defun %split-lines (text)
  (let ((parts '())
        (start 0))
    (loop for i = (position #\Newline text :start start)
          do (push (subseq text start (or i (length text))) parts)
          if (null i) do (return)
          do (setf start (1+ i)))
    (remove-if #'(lambda (s) (zerop (length s))) (nreverse parts))))

;;; --- Vault port ---

(defparameter *vault-lib* nil)

(cffi:defcfun ("harmonia_vault_init" %vault-init) :int)
(cffi:defcfun ("harmonia_vault_set_secret" %vault-set-secret) :int
  (symbol :string)
  (value :string))
(cffi:defcfun ("harmonia_vault_has_secret" %vault-has-secret) :int
  (symbol :string))
(cffi:defcfun ("harmonia_vault_list_symbols" %vault-list-symbols) :pointer)
(cffi:defcfun ("harmonia_vault_last_error" %vault-last-error) :pointer)
(cffi:defcfun ("harmonia_vault_free_string" %vault-free-string) :void
  (ptr :pointer))

(defun init-vault-port ()
  (setf *vault-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_vault.dylib")))
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

(defun vault-has-secret-p (symbol)
  (let ((rc (%vault-has-secret symbol)))
    (cond
      ((minusp rc) (error "Vault has-secret failed: ~A" (vault-last-error)))
      ((zerop rc) nil)
      (t t))))

(defun vault-list-symbols ()
  (let ((ptr (%vault-list-symbols)))
    (if (cffi:null-pointer-p ptr)
        '()
        (unwind-protect
             (let ((raw (cffi:foreign-string-to-lisp ptr)))
               (if (zerop (length raw))
                   '()
                   (%split-lines raw)))
          (%vault-free-string ptr)))))
