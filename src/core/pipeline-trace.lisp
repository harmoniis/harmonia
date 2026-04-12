;;; pipeline-trace.lisp — Detailed pipeline tracing for Harmonia.
;;;
;;; Writes structured trace events to ~/.harmoniis/harmonia/pipeline-trace.jsonl
;;; Each event is a JSON line with timestamp, stage, and structured metadata.
;;; Enable with (setf *pipeline-trace-enabled* t) or env HARMONIA_PIPELINE_TRACE=1.

(in-package :harmonia)

(defvar *pipeline-trace-enabled*
  (string= (or (sb-ext:posix-getenv "HARMONIA_PIPELINE_TRACE") "") "1")
  "When T, write detailed pipeline trace events to disk.")

(defvar *pipeline-trace-file* nil
  "Stream for the pipeline trace output file.")

(defvar *pipeline-trace-path*
  (merge-pathnames "pipeline-trace.jsonl"
                   (make-pathname :directory
                     (pathname-directory
                       (or (sb-ext:posix-getenv "HARMONIA_STATE_ROOT")
                           (merge-pathnames ".harmoniis/harmonia/"
                             (user-homedir-pathname))))))
  "Path to the pipeline trace JSONL file.")

(defvar *pipeline-trace-seq* 0
  "Monotonically increasing sequence number for trace events.")

(defun pipeline-trace-enable ()
  "Enable pipeline tracing. Opens trace file if not open."
  (setf *pipeline-trace-enabled* t)
  (unless *pipeline-trace-file*
    (ensure-directories-exist *pipeline-trace-path*)
    (setf *pipeline-trace-file*
          (open *pipeline-trace-path*
                :direction :output
                :if-exists :append
                :if-does-not-exist :create))
    (%log :info "pipeline-trace" "Tracing enabled → ~A" *pipeline-trace-path*))
  t)

(defun pipeline-trace-disable ()
  "Disable pipeline tracing."
  (setf *pipeline-trace-enabled* nil)
  (when *pipeline-trace-file*
    (close *pipeline-trace-file*)
    (setf *pipeline-trace-file* nil))
  t)

(defun %pipeline-trace (stage &rest kvs)
  "Write a structured trace event. STAGE is a keyword, KVS is a plist."
  (when *pipeline-trace-enabled*
    (handler-case
        (progn
          (unless *pipeline-trace-file* (pipeline-trace-enable))
          (incf *pipeline-trace-seq*)
          (let ((json (format nil "{\"seq\":~D,\"ts\":~D,\"stage\":\"~A\"~{,\"~A\":~A~}}"
                        *pipeline-trace-seq*
                        (get-universal-time)
                        (string-downcase (symbol-name stage))
                        (loop for (k v) on kvs by #'cddr
                              collect (string-downcase (symbol-name k))
                              collect (%trace-json-value v)))))
            (write-line json *pipeline-trace-file*)
            (force-output *pipeline-trace-file*)))
      (error () nil))))

(defun %trace-json-value (v)
  "Convert a Lisp value to JSON representation."
  (typecase v
    (null "null")
    (string (format nil "\"~A\"" (%trace-json-escape v)))
    (integer (format nil "~D" v))
    (float (format nil "~,6F" v))
    (double-float (format nil "~,6F" v))
    (keyword (format nil "\"~A\"" (string-downcase (symbol-name v))))
    (symbol (format nil "\"~A\"" (string-downcase (symbol-name v))))
    (cons (format nil "\"~A\"" (%trace-json-escape (format nil "~S" v))))
    (t (format nil "\"~A\"" (%trace-json-escape (princ-to-string v))))))

(defun %trace-json-escape (s)
  "Escape a string for JSON embedding."
  (let ((out (make-string-output-stream)))
    (loop for c across (or s "")
          do (case c
               (#\" (write-string "\\\"" out))
               (#\\ (write-string "\\\\" out))
               (#\Newline (write-string "\\n" out))
               (#\Return (write-string "\\r" out))
               (#\Tab (write-string "\\t" out))
               (t (write-char c out))))
    (get-output-stream-string out)))

;;; ──────────────────────────────────────────────────────────────────────
;;; STAGE-SPECIFIC TRACE FUNCTIONS
;;; Called from the pipeline at each critical point.
;;; ──────────────────────────────────────────────────────────────────────

(defun %trace-gateway-ingestion (envelope)
  "Trace signal arrival at gateway."
  (%pipeline-trace :gateway-ingestion
    :frontend (or (getf (getf envelope :channel) :kind) "?")
    :address (or (getf (getf envelope :channel) :address) "?")
    :security-label (or (getf (getf envelope :security) :label) "?")
    :peer-device (or (getf (getf envelope :peer) :device-id) "?")
    :payload-len (length (or (getf (getf envelope :body) :text) ""))
    :dissonance (or (getf (getf envelope :audit) :dissonance) 0.0)
    :prompt-preview (%clip-prompt (or (getf (getf envelope :body) :text) "") 80)))

(defun %trace-signal-constructed (signal)
  "Trace the fully constructed harmonia-signal."
  (when (harmonia-signal-p signal)
    (%pipeline-trace :signal-constructed
      :frontend (harmonia-signal-frontend signal)
      :security (harmonia-signal-security-label signal)
      :peer-id (harmonia-peer-id (harmonia-signal-peer signal))
      :conversation-id (harmonia-signal-conversation-id signal)
      :taint (harmonia-signal-taint signal)
      :dissonance (harmonia-signal-dissonance signal)
      :payload-len (length (or (harmonia-signal-payload signal) "")))))

(defun %trace-complexity-encoding (prompt dimensions tier)
  "Trace the complexity encoder's assessment."
  (%pipeline-trace :complexity-encoding
    :prompt-len (length (or prompt ""))
    :tier tier
    :dimensions dimensions))

(defun %trace-model-selection (model tier pool-size reason prompt)
  "Trace model routing decision."
  (%pipeline-trace :model-selection
    :model model
    :tier (string-downcase (symbol-name tier))
    :pool-size pool-size
    :reason reason
    :prompt-len (length (or prompt ""))))

(defun %trace-conductor-decision (path model prompt reason)
  "Trace the conductor's routing decision."
  (%pipeline-trace :conductor-decision
    :path (string-downcase (symbol-name path))
    :model (or model "none")
    :reason reason
    :prompt-preview (%clip-prompt prompt 60)))

(defun %trace-repl-round (round-num input-len model response-type response-len)
  "Trace each REPL evaluation round."
  (%pipeline-trace :repl-round
    :round round-num
    :input-len input-len
    :model (or model "?")
    :response-type response-type
    :response-len response-len))

(defun %trace-memory-recall (source query count chars-used)
  "Trace memory recall operations."
  (%pipeline-trace :memory-recall
    :source source
    :query (%clip-prompt query 60)
    :count count
    :chars-used chars-used))

(defun %trace-memory-store (entry-type tags)
  "Trace memory storage."
  (%pipeline-trace :memory-store
    :entry-type (string-downcase (symbol-name entry-type))
    :tags (format nil "~{~A~^ ~}" (or tags '()))))

(defun %trace-matrix-route (from to allowed signal noise)
  "Trace matrix routing decisions."
  (%pipeline-trace :matrix-route
    :from from :to to
    :allowed allowed
    :signal signal :noise noise))

(defun %trace-signalograd-step (cycle projection-confidence stability novelty)
  "Trace signalograd kernel stepping."
  (%pipeline-trace :signalograd-step
    :cycle cycle
    :confidence projection-confidence
    :stability stability
    :novelty novelty))

(defun %trace-response-delivery (frontend response-len model latency-ms)
  "Trace final response delivery."
  (%pipeline-trace :response-delivery
    :frontend frontend
    :response-len response-len
    :model (or model "?")
    :latency-ms latency-ms))

(defun %trace-llm-call (backend model latency-ms success)
  "Trace individual LLM API calls."
  (%pipeline-trace :llm-call
    :backend backend
    :model model
    :latency-ms latency-ms
    :success success))

(defun %trace-swarm-spawn (model actor-id task-preview)
  "Trace tmux subagent spawning."
  (%pipeline-trace :swarm-spawn
    :model model
    :actor-id actor-id
    :task-preview (%clip-prompt task-preview 60)))
