;;; git-ops.lisp — CFFI bridge for harmonia_git_ops dylib.

(in-package :harmonia)

(defparameter *git-ops-lib* nil)

(cffi:defcfun ("harmonia_git_ops_commit_all" %git-commit-all) :int
  (repo :string)
  (message :string)
  (author-name :string)
  (author-email :string))
(cffi:defcfun ("harmonia_git_ops_push" %git-push) :int
  (repo :string)
  (remote :string)
  (branch :string))
(cffi:defcfun ("harmonia_git_ops_last_error" %git-last-error) :pointer)
(cffi:defcfun ("harmonia_git_ops_free_string" %git-free-string) :void
  (ptr :pointer))

(defun init-git-ops-backend ()
  (ensure-cffi)
  (setf *git-ops-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_git_ops.dylib")))
  t)

(defun git-ops-last-error ()
  (let ((ptr (%git-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%git-free-string ptr)))))

(defun git-commit-and-push (repo branch message &key (remote "origin")
                             (author-name "Harmonia Agent")
                             (author-email "harmonia@test.local"))
  (let ((commit-rc (%git-commit-all repo message author-name author-email)))
    (unless (zerop commit-rc)
      (error "git commit failed: ~A" (git-ops-last-error)))
    (let ((push-rc (%git-push repo remote branch)))
      (unless (zerop push-rc)
        (error "git push failed: ~A" (git-ops-last-error)))
      t)))
