;;; gitop.lisp — Port: Git operations via IPC to the git-ops actor.
;;;
;;; Provides git status, log, diff, commit, push to Lisp and the REPL.
;;; The Rust git-ops actor shells out to git — security is in the policy gate.

(in-package :harmonia)

(defun init-gitop-port ()
  "Initialize git-ops port. Verifies the actor responds."
  (let ((reply (ignore-errors
                 (ipc-call "(:component \"git-ops\" :op \"healthcheck\")"))))
    (if (and reply (ipc-reply-ok-p reply))
        (progn (%log :info "gitop" "git-ops actor ready")
               t)
        (progn (%log :warn "gitop" "git-ops actor not available")
               nil))))

(defun %gitop-repo-path ()
  "Return the repo path. Uses config or falls back to state root."
  (or (ignore-errors (config-get-for "git-ops" "repo-path"))
      (ignore-errors (funcall 'state-root))
      "."))

(defun %gitop-call (op &rest extra-pairs)
  "Call a git-ops IPC operation. Returns the :result string or error."
  (let* ((repo (%gitop-repo-path))
         (sexp (format nil "(:component \"git-ops\" :op \"~A\" :repo \"~A\"~{ ~A~})"
                       (sexp-escape-lisp op) (sexp-escape-lisp repo) extra-pairs))
         (reply (ipc-call sexp)))
    (if (and reply (ipc-reply-ok-p reply))
        (let* ((*read-eval* nil)
               (parsed (ignore-errors (read-from-string reply))))
          (or (and (listp parsed) (getf (cdr parsed) :result))
              reply))
        (or reply "(git-ops: no response)"))))

;;; ─── Public API ────────────────────────────────────────────────────

(defun git-status ()
  "Return git status --porcelain output."
  (%gitop-call "status"))

(defun git-log (&optional (limit 10))
  "Return last N commits (oneline format)."
  (%gitop-call "log" (format nil ":limit ~D" limit)))

(defun git-diff ()
  "Return git diff --stat summary."
  (%gitop-call "diff"))

(defun git-diff-full ()
  "Return full git diff (truncated to 4000 chars by actor)."
  (%gitop-call "diff-full"))

(defun git-branch ()
  "Return all branches."
  (%gitop-call "branch"))

(defun git-branch-current ()
  "Return the current branch name."
  (%gitop-call "branch-current"))

(defun git-commit (message &key (author "Harmonia") (email "harmonia@local.invalid"))
  "Stage all and commit with MESSAGE."
  (%gitop-call "commit"
    (format nil ":message \"~A\"" (sexp-escape-lisp message))
    (format nil ":author \"~A\"" (sexp-escape-lisp author))
    (format nil ":email \"~A\"" (sexp-escape-lisp email))))

(defun git-push (&key (remote "origin") (branch "main"))
  "Push HEAD to REMOTE/BRANCH."
  (%gitop-call "push"
    (format nil ":remote \"~A\"" (sexp-escape-lisp remote))
    (format nil ":branch \"~A\"" (sexp-escape-lisp branch))))
