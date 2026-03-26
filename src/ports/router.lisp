;;; router.lisp — Port: generic LLM provider routing via IPC.

(in-package :harmonia)

(defun init-router-port ()
  (let ((reply (ipc-call "(:component \"provider-router\" :op \"healthcheck\")")))
    (let ((healthy (and reply (search ":healthy t" reply))))
      (%log :info "router" "Provider router initialized (healthy=~A)" healthy)
      (runtime-log *runtime* :native-init (list :provider-router (if healthy :ok :degraded)))
      healthy)))

(defun router-healthcheck ()
  (let ((reply (ipc-call "(:component \"provider-router\" :op \"healthcheck\")")))
    (and reply (search ":healthy t" reply))))

(defun backend-last-error ()
  "")

(defun backend-complete (prompt &optional model)
  (let* ((model-str (or model ""))
         (reply (ipc-call
                 (format nil "(:component \"provider-router\" :op \"complete\" :prompt \"~A\" :model \"~A\")"
                         (sexp-escape-lisp prompt) (sexp-escape-lisp model-str)))))
    (if (and reply (ipc-reply-ok-p reply))
        (or (ipc-extract-value reply) "")
        (error "LLM request failed: ~A" (or reply "IPC unreachable")))))

(defun backend-complete-safe (prompt &optional model)
  "Like backend-complete but returns NIL on failure instead of signaling."
  (handler-case (backend-complete prompt model)
    (error () nil)))

(defun backend-complete-for-task (prompt task-hint)
  (let ((reply (ipc-call
                (format nil "(:component \"provider-router\" :op \"complete-for-task\" :prompt \"~A\" :task \"~A\")"
                        (sexp-escape-lisp prompt) (sexp-escape-lisp task-hint)))))
    (if (and reply (ipc-reply-ok-p reply))
        (or (ipc-extract-value reply) "")
        (error "LLM request failed: ~A" (or reply "IPC unreachable")))))

(defun backend-list-models ()
  (let ((reply (ipc-call "(:component \"provider-router\" :op \"list-models\")")))
    (or (and reply (ipc-extract-value reply)) "")))

(defun backend-select-model (task-hint)
  (let ((reply (ipc-call
                (format nil "(:component \"provider-router\" :op \"select-model\" :task \"~A\")"
                        (sexp-escape-lisp task-hint)))))
    (or (and reply (ipc-extract-value reply)) "")))

(defun backend-list-backends ()
  (let ((reply (ipc-call "(:component \"provider-router\" :op \"list-backends\")")))
    (when (and reply (ipc-reply-ok-p reply))
      (ipc-extract-value reply))))

(defun backend-backend-status (&optional (name ""))
  (let ((reply (ipc-call
                (format nil "(:component \"provider-router\" :op \"backend-status\" :name \"~A\")"
                        (sexp-escape-lisp name)))))
    (when (and reply (ipc-reply-ok-p reply))
      (ipc-extract-value reply))))
