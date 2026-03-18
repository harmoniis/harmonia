;;; observability.lisp — Port: distributed tracing via LangSmith CFFI.
;;;
;;; Non-blocking background trace submission. All trace calls are no-ops when
;;; observability is disabled or the API key is not configured.
;;; Observability must NEVER block the agent — all errors are silently ignored.

(in-package :harmonia)

(defparameter *observability-lib* nil)
(defparameter *observability-initialized* nil)

;;; --- CFFI declarations ---

(cffi:defcfun ("harmonia_observability_init" %observability-init) :int
  (config-sexp :string))

(cffi:defcfun ("harmonia_observability_trace_start" %observability-trace-start) :int64
  (name :string)
  (kind :string)
  (parent-id :int64)
  (metadata-sexp :string))

(cffi:defcfun ("harmonia_observability_trace_end" %observability-trace-end) :int
  (trace-handle :int64)
  (status :string)
  (output-sexp :string))

(cffi:defcfun ("harmonia_observability_trace_event" %observability-trace-event) :int
  (name :string)
  (kind :string)
  (metadata-sexp :string))

(cffi:defcfun ("harmonia_observability_flush" %observability-flush) :int)

(cffi:defcfun ("harmonia_observability_shutdown" %observability-shutdown) :int)

;;; --- Port API ---

(defun init-observability-port ()
  "Load the observability dylib and initialize tracing.
   Non-fatal on failure — the agent runs without tracing."
  (handler-case
      (progn
        (ensure-cffi)
        (setf *observability-lib*
              (cffi:load-foreign-library
               (%release-lib-path "libharmonia_observability.dylib")))
        (let ((rc (%observability-init "")))
          (unless (zerop rc)
            (%log :warn "observability" "init returned non-zero: ~D" rc)))
        (setf *observability-initialized* t)
        ;; Register as actor through the unified registry
        (when *runtime*
          (ignore-errors
            (let ((actor-id (actor-register "observability")))
              (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "observability"))))
        (%log :info "observability" "Initialized.")
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
      (%observability-trace-start
       (or name "unknown")
       (string-downcase (symbol-name (or kind :chain)))
       (or parent-id 0)
       (if metadata (format nil "~S" metadata) "")))))

(defun trace-end (handle &key (status :success) output)
  "End a trace span. HANDLE 0 = no-op."
  (when (and *observability-initialized* handle (plusp handle))
    (ignore-errors
      (%observability-trace-end
       handle
       (string-downcase (symbol-name (or status :success)))
       (if output (format nil "~S" output) "")))))

(defun trace-event (name kind &key metadata)
  "Fire-and-forget trace event."
  (when *observability-initialized*
    (ignore-errors
      (%observability-trace-event
       (or name "unknown")
       (string-downcase (symbol-name (or kind :chain)))
       (if metadata (format nil "~S" metadata) "")))))

(defun trace-flush ()
  "Flush pending traces to LangSmith."
  (when *observability-initialized*
    (ignore-errors (%observability-flush))))

(defun trace-shutdown ()
  "Shut down the observability subsystem."
  (when *observability-initialized*
    (ignore-errors (%observability-shutdown))
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
