;;; observability.lisp — Port: distributed tracing via IPC.
;;;
;;; Non-blocking background trace submission. All trace calls are no-ops when
;;; observability is disabled or the API key is not configured.
;;; Observability must NEVER block the agent — all errors are silently ignored.

(in-package :harmonia)

(defparameter *observability-initialized* nil)

;;; --- Trace-level filtering ---

(defparameter *observability-trace-level* :standard
  "Current trace level: :minimal, :standard, or :verbose.
   Controls which trace events are emitted.")

(defun %trace-level-p (required)
  "Return T if current trace-level >= required.
   :verbose >= :standard >= :minimal"
  (case *observability-trace-level*
    (:verbose t)
    (:standard (member required '(:minimal :standard)))
    (:minimal (eq required :minimal))
    (t nil)))

;;; --- Parent-child trace correlation ---

(defparameter *current-trace-handle* 0
  "Dynamic variable holding the active trace span handle.
   Child trace-event calls use this as :parent-id for correlation.
   Bound by with-trace; 0 means no active parent span.")

;;; --- Port API ---

(defun init-observability-port ()
  "Initialize observability tracing via IPC.
   Non-fatal on failure — the agent runs without tracing."
  (handler-case
      (progn
        (let ((reply (ipc-call "(:component \"observability\" :op \"init\")")))
          (when (and reply (ipc-reply-ok-p reply))
            (setf *observability-initialized* t)))
        ;; Load trace level from config store
        (when *observability-initialized*
          (ignore-errors
            (let ((level (config-get-for "observability" "trace-level")))
              (when level
                (setf *observability-trace-level*
                      (cond
                        ((string-equal level "minimal") :minimal)
                        ((string-equal level "verbose") :verbose)
                        (t :standard)))))))
        ;; Register as actor through the unified registry
        (when (and *observability-initialized* *runtime*)
          (ignore-errors
            (let ((actor-id (actor-register "observability")))
              (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "observability"))))
        (if *observability-initialized*
            (%log :info "observability"
                  "Initialized via IPC (level=~A)."
                  *observability-trace-level*)
            (%log :info "observability" "Not available (non-fatal)."))
        t)
    (error (e)
      (%log :warn "observability" "Init failed (non-fatal): ~A" e)
      nil)))

;;; --- Public tracing API ---

(defun trace-start (name kind &key (parent-id 0) metadata)
  "Start a new trace span. Returns a handle (>0) or 0 if disabled.
   KIND is one of: :chain :llm :tool :agent
   PARENT-ID: use *current-trace-handle* for automatic parent correlation."
  (when *observability-initialized*
    (ignore-errors
      (let* ((pid (if (and parent-id (plusp parent-id))
                      parent-id
                      (if (plusp *current-trace-handle*)
                          *current-trace-handle*
                          0)))
             (reply (ipc-call
                     (format nil "(:component \"observability\" :op \"trace-start\" :name \"~A\" :kind \"~A\" :parent-id ~D :metadata \"~A\")"
                             (sexp-escape-lisp (or name "unknown"))
                             (string-downcase (symbol-name (or kind :chain)))
                             pid
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
  "Fire-and-forget trace event. Inherits parent from *current-trace-handle*."
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

(defmacro with-trace ((name &key (kind :chain) metadata) &body body)
  "Execute BODY within a traced span. Automatically ends the span on success or error.
   Binds *current-trace-handle* so child trace-event calls correlate.
   Usage: (with-trace (\"orchestrate-signal\" :kind :chain :metadata (:frontend fe))
            (body ...))"
  (let ((handle (gensym "TRACE-HANDLE-"))
        (result (gensym "TRACE-RESULT-"))
        (errorp (gensym "TRACE-ERROR-")))
    `(let* ((,handle (trace-start ,name ,kind
                                  :parent-id *current-trace-handle*
                                  :metadata ,metadata))
            (*current-trace-handle* (or ,handle 0))
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
