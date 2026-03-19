;;; router.lisp — Port: generic LLM provider routing.
;;;
;;; NOTE: The provider router is not yet wired as an IPC component.
;;; All wrappers return sensible defaults and log warnings until
;;; the Rust provider-router actor is connected to the IPC dispatch.

(in-package :harmonia)

(defun %parse-router-sexp (text)
  (when (and text (> (length text) 0) (not (string= text "nil")))
    (let ((*read-eval* nil))
      (ignore-errors (read-from-string text)))))

(defun init-router-port ()
  "No-op: provider router will be initialized when IPC component is wired."
  (%log :info "router" "Provider router port initialized (IPC stub — not yet wired)")
  (runtime-log *runtime* :native-init (list :provider-router :ipc-stub))
  t)

(defun router-healthcheck ()
  (%log :warn "router" "healthcheck called on unwired IPC stub")
  nil)

(defun backend-last-error ()
  "provider-router: not yet wired as IPC component")

(defun backend-complete (prompt &optional model)
  (declare (ignorable prompt model))
  (%log :warn "router" "backend-complete called on unwired IPC stub (model=~A)" (or model "auto"))
  (error "LLM request failed: provider-router not yet wired as IPC component"))

(defun backend-complete-for-task (prompt task-hint)
  (declare (ignorable prompt task-hint))
  (%log :warn "router" "backend-complete-for-task called on unwired IPC stub")
  (error "LLM request failed: provider-router not yet wired as IPC component"))

(defun backend-list-models ()
  (%log :warn "router" "backend-list-models called on unwired IPC stub")
  "")

(defun backend-select-model (task-hint)
  (declare (ignorable task-hint))
  (%log :warn "router" "backend-select-model called on unwired IPC stub")
  "")

(defun backend-list-backends ()
  (%log :warn "router" "backend-list-backends called on unwired IPC stub")
  nil)

(defun backend-backend-status (&optional (name ""))
  (declare (ignorable name))
  (%log :warn "router" "backend-backend-status called on unwired IPC stub")
  nil)
