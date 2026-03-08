;;; state.lisp — Runtime state container.

(in-package :harmonia)

;;; --- Security kernel: typed signal struct ---

(defstruct harmonia-signal
  id               ;; u64 from gateway
  frontend         ;; string: "telegram", "mqtt", etc.
  sub-channel      ;; string: channel/chat ID
  security-label   ;; keyword: :owner/:authenticated/:anonymous/:untrusted
  payload          ;; string: message content (UNTRUSTED for external)
  capabilities     ;; string or nil
  metadata         ;; string or nil
  timestamp-ms     ;; integer
  taint            ;; keyword: :external/:tool-output/:memory-recall/:internal
  dissonance       ;; float 0.0-1.0 from injection scan
  origin-fp)       ;; string or nil: fingerprint from MQTT/tailnet

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
