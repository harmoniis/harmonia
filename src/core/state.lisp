;;; state.lisp — Runtime state container.

(in-package :harmonia)

;;; --- Security kernel: typed baseband channel envelope ---

(defstruct harmonia-channel
  kind
  address
  label)

(defstruct harmonia-peer
  id
  origin-fp
  agent-fp
  device-id
  platform
  device-model
  app-version
  a2ui-version)

(defstruct harmonia-body
  format
  text
  raw)

(defstruct harmonia-security
  label
  source
  fingerprint-valid-p)

(defstruct harmonia-audit
  timestamp-ms
  dissonance)

(defstruct harmonia-transport
  kind
  raw-address
  raw-metadata)

(defstruct harmonia-signal
  id
  version
  kind
  type-name
  channel
  peer
  conversation-id
  body
  capabilities
  security
  audit
  attachments
  transport
  taint)

(defun harmonia-signal-channel-kind (signal)
  (and signal (harmonia-channel-kind (harmonia-signal-channel signal))))

(defun harmonia-signal-channel-address (signal)
  (and signal (harmonia-channel-address (harmonia-signal-channel signal))))

(defun harmonia-signal-channel-label (signal)
  (and signal (harmonia-channel-label (harmonia-signal-channel signal))))

(defun harmonia-signal-frontend (signal)
  (harmonia-signal-channel-kind signal))

(defun harmonia-signal-sub-channel (signal)
  (harmonia-signal-channel-address signal))

(defun harmonia-signal-payload (signal)
  (and signal (harmonia-body-text (harmonia-signal-body signal))))

(defun harmonia-signal-security-label (signal)
  (and signal (harmonia-security-label (harmonia-signal-security signal))))

(defun harmonia-signal-dissonance (signal)
  (and signal (harmonia-audit-dissonance (harmonia-signal-audit signal))))

(defun harmonia-signal-timestamp-ms (signal)
  (and signal (harmonia-audit-timestamp-ms (harmonia-signal-audit signal))))

(defun harmonia-signal-origin-fp (signal)
  (and signal (harmonia-peer-origin-fp (harmonia-signal-peer signal))))

(defun harmonia-signal-agent-fp (signal)
  (and signal (harmonia-peer-agent-fp (harmonia-signal-peer signal))))

(defun harmonia-signal-device-id (signal)
  (and signal (harmonia-peer-device-id (harmonia-signal-peer signal))))

(defun harmonia-signal-platform (signal)
  (and signal (harmonia-peer-platform (harmonia-signal-peer signal))))

(defun harmonia-signal-a2ui-version (signal)
  (and signal (harmonia-peer-a2ui-version (harmonia-signal-peer signal))))

(defun harmonia-signal-has-capability-p (signal capability)
  (let ((caps (harmonia-signal-capabilities signal)))
    (and (listp caps)
         (let ((probe (intern (string-upcase capability) :keyword)))
           (or (getf caps probe)
               (getf caps capability))))))

(defvar *current-originating-signal* nil
  "The signal that initiated the current orchestration chain. Used by policy gate.")

(defstruct (runtime-state
             (:constructor make-runtime-state
                           (&key
                            (running t)
                            (cycle 0)
                            (started-at (get-universal-time))
                            (last-tick-at nil)
                            (tools (make-hash-table :test 'equal))
                            (events '())
                            (prompt-queue '())
                            (responses '())
                            (rewrite-count 0)
                            (environment "test")
                            (active-model nil)
                            (harmonic-phase :observe)
                            (harmonic-context '())
                            (harmonic-x 0.5)
                            (harmonic-r 3.45)
                            (lorenz-x 0.1)
                            (lorenz-y 0.0)
                            (lorenz-z 0.0))))
  running
  cycle
  started-at
  last-tick-at
  tools
  events
  prompt-queue
  responses
  rewrite-count
  environment
  active-model
  harmonic-phase
  harmonic-context
  harmonic-x
  harmonic-r
  lorenz-x
  lorenz-y
  lorenz-z)

(defun runtime-log (runtime tag payload)
  (push (list :time (get-universal-time) :tag tag :payload payload)
        (runtime-state-events runtime))
  runtime)
