;;; observability.lisp — Port: distributed tracing via IPC.
;;;
;;; Non-blocking background trace submission. All trace calls are no-ops when
;;; observability is disabled or the API key is not configured.
;;; Observability must NEVER block the agent — all errors are silently ignored.
;;;
;;; NOTE: observability is not yet wired as an IPC component.
;;; Trace calls are no-ops until the Rust actor is connected.

(in-package :harmonia)

(defparameter *observability-initialized* nil)

;;; --- Port API ---

(defun init-observability-port ()
  "Initialize observability tracing via IPC.
   Non-fatal on failure — the agent runs without tracing."
  (handler-case
      (progn
        (let ((reply (ipc-call "(:component \"observability\" :op \"init\")")))
          (when (and reply (ipc-reply-ok-p reply))
            (setf *observability-initialized* t)))
        ;; Register as actor through the unified registry
        (when (and *observability-initialized* *runtime*)
          (ignore-errors
            (let ((actor-id (actor-register "observability")))
              (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "observability"))))
        (if *observability-initialized*
            (%log :info "observability" "Initialized via IPC.")
            (%log :info "observability" "Not available (IPC component not wired — non-fatal)."))
        t)
    (error (e)
      (%log :warn "observability" "Init failed (non-fatal): ~A" e)
      nil)))

;;; --- Public tracing API ---

(defun trace-start (name kind &key (parent-id 0) metadata)
  "Start a new trace span. Returns a handle (>0) or 0 if disabled.
   KIND is one of: :chain :llm :tool :agent"
  (when *observability-initialized*
    (ignore-errors
      (let ((reply (ipc-call
                    (format nil "(:component \"observability\" :op \"trace-start\" :name \"~A\" :kind \"~A\" :parent-id ~D :metadata \"~A\")"
                            (sexp-escape-lisp (or name "unknown"))
                            (string-downcase (symbol-name (or kind :chain)))
                            (or parent-id 0)
                            (sexp-escape-lisp (if metadata (format nil "~S" metadata) ""))))))
        (or (ipc-extract-u64 reply ":handle") 0)))))

(defun trace-end (handle &key (status :success) output)
  "End a trace span. HANDLE 0 = no-op."
  (when (and *observability-initialized* handle (plusp handle))
    (ignore-errors
      (ipc-call
       (format nil "(:component \"observability\" :op \"trace-end\" :handle ~D :status \"~A\" :output \"~A\")"
               handle
               (string-downcase (symbol-name (or status :success)))
               (sexp-escape-lisp (if output (format nil "~S" output) "")))))))

(defun trace-event (name kind &key metadata)
  "Fire-and-forget trace event."
  (when *observability-initialized*
    (ignore-errors
      (ipc-call
       (format nil "(:component \"observability\" :op \"trace-event\" :name \"~A\" :kind \"~A\" :metadata \"~A\")"
               (sexp-escape-lisp (or name "unknown"))
               (string-downcase (symbol-name (or kind :chain)))
               (sexp-escape-lisp (if metadata (format nil "~S" metadata) "")))))))

(defun trace-flush ()
  "Flush pending traces."
  (when *observability-initialized*
    (ignore-errors
      (ipc-call "(:component \"observability\" :op \"flush\")"))))

(defun trace-shutdown ()
  "Shut down the observability subsystem."
  (when *observability-initialized*
    (ignore-errors
      (ipc-call "(:component \"observability\" :op \"shutdown\")"))
    (setf *observability-initialized* nil)))

;;; --- with-trace macro ---

(defmacro with-trace ((name &key (kind :chain) (parent-id 0) metadata) &body body)
  "Execute BODY within a traced span. Automatically ends the span on success or error.
   Usage: (with-trace (\"orchestrate-signal\" :kind :chain :metadata (:frontend fe))
            (body ...))"
  (let ((handle (gensym "TRACE-HANDLE-"))
        (result (gensym "TRACE-RESULT-"))
        (errorp (gensym "TRACE-ERROR-")))
    `(let ((,handle (trace-start ,name ,kind :parent-id ,parent-id :metadata ,metadata))
           (,errorp nil)
           (,result nil))
       (unwind-protect
            (handler-case
                (setf ,result (progn ,@body))
              (error (e)
                (setf ,errorp t)
                (trace-end ,handle :status :error
                           :output (list :error (princ-to-string e)))
                (error e)))
         (unless ,errorp
           (trace-end ,handle :status :success)))
       ,result)))
