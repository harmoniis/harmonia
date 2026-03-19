;;; lineage.lisp — Port: VCS commit/push operations.
;;;
;;; NOTE: git-ops is not yet wired as an IPC component.
;;; Wrappers return errors until the Rust actor is connected.

(in-package :harmonia)

(defun init-lineage-port ()
  "No-op: lineage (git-ops) will be initialized when IPC component is wired."
  (%log :info "lineage" "Lineage port initialized (IPC stub — not yet wired)")
  t)

(defun git-ops-last-error ()
  "git-ops: not yet wired as IPC component")

(defun git-commit-and-push (repo branch message &key (remote "origin")
                             (author-name "Harmonia Agent")
                             (author-email "harmonia@test.local"))
  (declare (ignorable repo branch message remote author-name author-email))
  (%log :warn "lineage" "git-commit-and-push called on unwired IPC stub")
  (error "git-commit-and-push: git-ops not yet wired as IPC component"))
