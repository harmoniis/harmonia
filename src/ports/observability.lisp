;;; observability.lisp — Port: distributed tracing via IPC.
;;;
;;; Non-blocking background trace submission. All trace calls are no-ops when
;;; observability is disabled or the API key is not configured.
;;; Observability must NEVER block the agent — all errors are silently ignored.
;;;
;;; Architecture: trace-start/trace-end/trace-event use ipc-cast (fire-and-forget).
;;; Run-ids are pre-generated client-side as UUID strings — no server round-trip.
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

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

(defparameter *current-trace-handle* ""
  "Dynamic variable holding the active trace span run-id string.
   Child trace-event calls use this as :parent-run-id for correlation.
   Bound by with-trace; empty string means no active parent span.")

;;; --- UUID generation ---

(defun %new-run-id ()
  "Generate a UUID v4 string for trace run-ids."
  (let ((bytes (make-array 16 :element-type '(unsigned-byte 8))))
    (with-open-file (s "/dev/urandom" :element-type '(unsigned-byte 8))
      (read-sequence bytes s))
    ;; Set UUID v4 variant bits
    (setf (aref bytes 6) (logior #x40 (logand (aref bytes 6) #x0f)))
    (setf (aref bytes 8) (logior #x80 (logand (aref bytes 8) #x3f)))
    (format nil "~(~2,'0x~2,'0x~2,'0x~2,'0x-~2,'0x~2,'0x-~2,'0x~2,'0x-~2,'0x~2,'0x-~2,'0x~2,'0x~2,'0x~2,'0x~2,'0x~2,'0x~)"
            (aref bytes 0) (aref bytes 1) (aref bytes 2) (aref bytes 3)
            (aref bytes 4) (aref bytes 5) (aref bytes 6) (aref bytes 7)
            (aref bytes 8) (aref bytes 9) (aref bytes 10) (aref bytes 11)
            (aref bytes 12) (aref bytes 13) (aref bytes 14) (aref bytes 15))))

;;; --- Port API ---

(defun init-observability-port ()
  "Initialize observability tracing via IPC.
   Non-fatal on failure — the agent runs without tracing."
  (handler-case
      (progn
        (let* ((reply (ipc-call (%sexp-to-ipc-string
                                  '(:component "observability" :op "init"))))
               (parsed (when (ipc-reply-ok-p reply) (ipc-parse-sexp-reply reply))))
          (when parsed
            (if (getf (cdr parsed) :enabled)
                (setf *observability-initialized* t)
                (%log :warn "observability"
                      "Rust init OK but tracing DISABLED — set LANGCHAIN_API_KEY"))))
        ;; Load trace level from config store
        (when *observability-initialized*
          (handler-case

              (let ((level (config-get-for "observability" "trace-level")

            (error () nil)))
              (when level
                (setf *observability-trace-level*
                      (cond
                        ((string-equal level "minimal") :minimal)
                        ((string-equal level "verbose") :verbose)
                        (t :standard)))))))
        ;; Register as actor through the unified registry
        (when (and *observability-initialized* *runtime*)
          (handler-case

              (let ((actor-id (actor-register "observability")

            (error () nil)))
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

(defun trace-start (name kind &key parent-run-id metadata)
  "Start a new trace span. Returns a run-id string, or \"\" if disabled.
   KIND is one of: :chain :llm :tool :agent
   PARENT-RUN-ID: string run-id from parent span, or nil."
  (when *observability-initialized*
    (handler-case

        (let* ((run-id (%new-run-id)

      (error () nil))
             (parent (or parent-run-id
                         (when (> (length *current-trace-handle*) 0)
                           *current-trace-handle*)))
             (cmd `(:component "observability" :op "trace-start"
                    :run-id ,run-id
                    :name ,(or name "unknown")
                    :kind ,(string-downcase (symbol-name (or kind :chain)))
                    ,@(when parent (list :parent-run-id parent))
                    :metadata ,(if metadata (format nil "~S" metadata) ""))))
        (ipc-cast (%sexp-to-ipc-string cmd))
        run-id))))

(defun trace-end (handle &key (status :success) output)
  "End a trace span. Empty string handle = no-op."
  (when (and *observability-initialized* handle (stringp handle) (> (length handle) 0))
    (handler-case

        (ipc-cast
       (%sexp-to-ipc-string
        `(:component "observability" :op "trace-end"
          :run-id ,handle
          :status ,(string-downcase (symbol-name (or status :success)

      (error () nil)))
          :output ,(if output (format nil "~S" output) "")))))))

(defun trace-event (name kind &key metadata)
  "Fire-and-forget trace event. Inherits parent from *current-trace-handle*."
  (when *observability-initialized*
    (handler-case

        (let* ((parent (when (> (length *current-trace-handle*) 0)
                       *current-trace-handle*)

      (error () nil))
             (cmd `(:component "observability" :op "trace-event"
                    :name ,(or name "unknown")
                    :kind ,(string-downcase (symbol-name (or kind :chain)))
                    ,@(when parent (list :parent-run-id parent))
                    :metadata ,(if metadata (format nil "~S" metadata) ""))))
        (ipc-cast (%sexp-to-ipc-string cmd))))))

(defun trace-flush ()
  "Flush pending traces."
  (when *observability-initialized*
    (handler-case

        (ipc-call (%sexp-to-ipc-string
                  '(:component "observability" :op "flush")

      (error () nil))))))

(defun trace-shutdown ()
  "Shut down the observability subsystem."
  (when *observability-initialized*
    (handler-case

        (ipc-call (%sexp-to-ipc-string
                  '(:component "observability" :op "shutdown")

      (error () nil))))
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
    `(let* ((,handle (trace-start ,name ,kind :metadata ,metadata))
            (*current-trace-handle* (or ,handle ""))
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
