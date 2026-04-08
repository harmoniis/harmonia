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

(defstruct harmonia-origin
  node-id
  node-label
  node-role
  channel-class
  node-key-id
  transport-security
  remote-p)

(defstruct harmonia-session
  id
  label)

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
  origin
  session
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

(defun harmonia-signal-origin-node-id (signal)
  (and signal (harmonia-origin-node-id (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-label (signal)
  (and signal (harmonia-origin-node-label (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-role (signal)
  (and signal (harmonia-origin-node-role (harmonia-signal-origin signal))))

(defun harmonia-signal-channel-class (signal)
  (and signal (harmonia-origin-channel-class (harmonia-signal-origin signal))))

(defun harmonia-signal-origin-node-key-id (signal)
  (and signal (harmonia-origin-node-key-id (harmonia-signal-origin signal))))

(defun harmonia-signal-transport-security (signal)
  (and signal (harmonia-origin-transport-security (harmonia-signal-origin signal))))

(defun harmonia-signal-remote-p (signal)
  (and signal (harmonia-origin-remote-p (harmonia-signal-origin signal))))

(defun harmonia-signal-session-id (signal)
  (and signal (harmonia-session-id (harmonia-signal-session signal))))

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

;;; --- Actor record: tracks a non-blocking CLI subagent ---

(defstruct actor-record
  id                        ;; u64 from Rust tmux-spawn
  model                     ;; "cli:claude-code"
  prompt                    ;; original prompt text
  state                     ;; :spawning :running :completed :failed :stalled
  spawned-at                ;; universal-time
  last-heartbeat            ;; universal-time of last progress
  originating-signal        ;; harmonia-signal or nil (for gateway delivery)
  result                    ;; string output when completed
  error-text                ;; string when failed
  cost-usd                  ;; float
  duration-ms               ;; integer
  stall-ticks               ;; count of ticks with no heartbeat
  orchestration-context     ;; plist with :chain, :prepared-prompt, etc.
  swarm-group-id            ;; groups actors from same parallel-solve invocation
  supervision-spec          ;; frozen supervision spec s-expression (or nil)
  supervision-grade         ;; :confirmed :partial :failed :deferred nil
  supervision-confidence)   ;; 0.0-1.0

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
                            (lorenz-z 0.0)
                            (actor-registry (make-hash-table :test 'eql))
                            (actor-pending '())
                            (actor-kinds (make-hash-table :test 'equal))
                            (chronicle-pending '())
                            (gateway-actor-id nil)
                            (tailnet-actor-id nil)
                            (chronicle-actor-id nil)
                            (response-seq 0)
                            (presentation-feedback '())
                            (last-response-telemetry '())
                            (signalograd-projection '())
                            (signalograd-last-updated-at 0))))
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
  lorenz-z
  actor-registry              ;; hash-table: actor-id -> actor-record
  actor-pending               ;; list of actor-ids awaiting completion
  actor-kinds                 ;; hash-table: actor-id -> kind string ("gateway", "cli-agent", etc.)
  chronicle-pending           ;; list of chronicle recording plists batched per tick
  gateway-actor-id            ;; actor-id of gateway actor (or nil)
  tailnet-actor-id            ;; actor-id of tailnet actor (or nil)
  chronicle-actor-id          ;; actor-id of chronicle actor (or nil)
  response-seq                ;; monotonically increasing internal response id
  presentation-feedback       ;; recent human feedback events (latest first)
  last-response-telemetry     ;; hidden telemetry sidecar for the last visible reply
  signalograd-projection      ;; last applied signalograd proposal plist
  signalograd-last-updated-at) ;; universal time of last applied proposal

(defun runtime-log (runtime tag payload)
  (push (list :time (get-universal-time) :tag tag :payload payload)
        (runtime-state-events runtime))
  runtime)
