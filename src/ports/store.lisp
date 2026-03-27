;;; store.lisp — Port: non-secret runtime configuration key-values via IPC.

(in-package :harmonia)

;;; --- Init ────────────────────────────────────────────────────────────

(defun init-store-port ()
  (let ((reply (ipc-config-init)))
    (runtime-log *runtime* :config-store-init
                 (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

;;; --- Error helper ────────────────────────────────────────────────────

(defun config-last-error ()
  "Config store errors are reported via IPC reply; this returns empty for compat."
  "")

;;; --- Simple wrappers (admin-level, no policy check) ──────────────────

(defun config-set (key value &optional (scope "global"))
  (let ((reply (ipc-config-set "admin" scope key (or value ""))))
    (when (ipc-reply-error-p reply)
      (error "Config store set failed: ~A" reply))
    t))

(defun config-get (key &optional (scope "global"))
  (ipc-config-get "admin" scope key))

(defun config-list (&optional (scope ""))
  (let ((reply (ipc-call
                (format nil "(:component \"config\" :op \"list\" :scope \"~A\")"
                        (sexp-escape-lisp (or scope ""))))))
    (if (and reply (ipc-reply-ok-p reply))
        (let ((val (ipc-extract-value reply)))
          (if (and val (> (length val) 0))
              (%split-lines val)
              '()))
        '())))

;;; --- Component-aware wrappers (policy-gated) ─────────────────────────

(defun config-get-for (component key &optional (scope component))
  "Get a config value with policy enforcement and env fallback chain."
  (ipc-config-get component scope key))

(defun config-get-or (component key default &optional (scope component))
  "Get a config value with default, policy enforcement and env fallback chain."
  (ipc-config-get-or component scope key (or default "")))

(defun config-set-for (component key value &optional (scope component))
  "Set a config value with policy enforcement."
  (let ((reply (ipc-config-set component scope key (or value ""))))
    (when (ipc-reply-error-p reply)
      (error "Config store set-for failed: ~A" reply))
    t))

(defun config-delete-for (component key &optional (scope component))
  "Delete a config value (admin-only via policy)."
  (let ((reply (ipc-call
                (format nil "(:component \"config\" :op \"delete\" :component \"~A\" :scope \"~A\" :key \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp scope)
                        (sexp-escape-lisp key)))))
    (when (ipc-reply-error-p reply)
      (error "Config store delete-for failed: ~A" reply))
    t))

(defun config-dump (component &optional (scope component))
  "Dump all key=value pairs in a scope as a list of lines."
  (let ((reply (ipc-call
                (format nil "(:component \"config\" :op \"dump\" :component \"~A\" :scope \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp scope)))))
    (if (and reply (ipc-reply-ok-p reply))
        (let ((val (ipc-extract-value reply)))
          (if (and val (> (length val) 0))
              (%split-lines val)
              '()))
        '())))

(defun config-ingest-env ()
  "Seed config DB from environment variables (first-run only)."
  (let ((reply (ipc-call "(:component \"config\" :op \"ingest-env\")")))
    (when (ipc-reply-error-p reply)
      (error "Config store ingest-env failed: ~A" reply))
    t))
