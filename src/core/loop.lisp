;;; loop.lisp — Core control loop with Erlang-style supervision.
;;;
;;; Design principle: the loop NEVER crashes. Individual actions may fail
;;; and are caught, recorded, and recovered from. Like an Erlang supervisor,
;;; the system gracefully degrades when subsystems fail, records the failure
;;; for self-diagnosis, and continues operating.
;;;
;;; Performance: zero overhead on the happy path. handler-case compiles to
;;; setjmp/longjmp in SBCL. No intermediate lists allocated per tick.
;;; Outbound queue uses atomic swap instead of copy+remove.

(in-package :harmonia)

;;; --- Queue operations (thread-safe for actor model) ---

(defvar %queue-lock (sb-thread:make-mutex :name "prompt-queue")
  "Lock for the shared prompt queue. Gateway and conductor actors
   access the queue from different threads.")

(defun %queue-pop (runtime)
  (sb-thread:with-mutex (%queue-lock)
    (let ((q (runtime-state-prompt-queue runtime)))
      (when q
        (setf (runtime-state-prompt-queue runtime) (rest q))
        (first q)))))

(defun %queue-push (runtime item)
  "Thread-safe enqueue to the back of the prompt queue."
  (sb-thread:with-mutex (%queue-lock)
    (setf (runtime-state-prompt-queue runtime)
          (nconc (runtime-state-prompt-queue runtime) (list item)))))

(defun %requeue-front (runtime prompt)
  (sb-thread:with-mutex (%queue-lock)
    (setf (runtime-state-prompt-queue runtime)
          (cons prompt (runtime-state-prompt-queue runtime)))))

;;; --- Helpers ---

(defun %prompt-for-log (prompt)
  "Extract a loggable representation from a prompt (string or harmonia-signal)."
  (if (harmonia-signal-p prompt)
      (format nil "[signal:~A/~A] ~A"
              (harmonia-signal-frontend prompt)
              (harmonia-signal-sub-channel prompt)
              (%clip-prompt (harmonia-signal-payload prompt)))
      prompt))

(defun %clip-prompt (text &optional (limit 256))
  (let ((s (or text "")))
    (if (<= (length s) limit) s (subseq s 0 limit))))

(defun %security-keyword-from-string (security)
  (cond
    ((string-equal security "owner") :owner)
    ((string-equal security "authenticated") :authenticated)
    ((string-equal security "anonymous") :anonymous)
    (t :untrusted)))

(defun %dissonance->injection-count (dissonance)
  (if (and (numberp dissonance) (> dissonance 0.0))
      (max 1 (round (/ (float dissonance) 0.15)))
      0))

(defun %make-harmonia-signal-from-envelope (envelope)
  (let* ((channel (getf envelope :channel))
         (peer (getf envelope :peer))
         (origin (getf envelope :origin))
         (session (getf envelope :session))
         (body (getf envelope :body))
         (security (getf envelope :security))
         (audit (getf envelope :audit)))
    (make-harmonia-signal
     :id (or (getf envelope :id) 0)
     :version (or (getf envelope :version) 1)
     :kind (or (getf envelope :kind) "external")
     :type-name (or (getf envelope :type-name) "message.text")
     :channel (make-harmonia-channel
               :kind (or (getf channel :kind) "unknown")
               :address (or (getf channel :address) "default")
               :label (or (getf channel :label)
                          (format nil "~A:~A"
                                  (or (getf channel :kind) "unknown")
                                  (or (getf channel :address) "default"))))
     :peer (make-harmonia-peer
            :id (or (getf peer :id)
                    (or (getf channel :label) "unknown"))
            :origin-fp (getf peer :origin-fp)
            :agent-fp (getf peer :agent-fp)
            :device-id (getf peer :device-id)
            :platform (getf peer :platform)
            :device-model (getf peer :device-model)
            :app-version (getf peer :app-version)
            :a2ui-version (getf peer :a2ui-version))
     :conversation-id (or (getf (getf envelope :conversation) :id)
                          (getf channel :label)
                          "default")
     :origin (when (listp origin)
               (make-harmonia-origin
                :node-id (getf origin :node-id)
                :node-label (getf origin :node-label)
                :node-role (getf origin :node-role)
                :channel-class (getf origin :channel-class)
                :node-key-id (getf origin :node-key-id)
                :transport-security (getf origin :transport-security)
                :remote-p (not (null (getf origin :remote)))))
     :session (when (listp session)
                (make-harmonia-session
                 :id (getf session :id)
                 :label (getf session :label)))
     :body (make-harmonia-body
            :format (or (getf body :format) "text")
            :text (or (getf body :text) "")
            :raw (or (getf body :raw) (or (getf body :text) "")))
     :capabilities (or (getf envelope :capabilities) '())
     :security (make-harmonia-security
                :label (%security-keyword-from-string
                        (or (getf security :label) "untrusted"))
                :source (or (getf security :source) "gateway")
                :fingerprint-valid-p (not (null (getf security :fingerprint-valid))))
     :audit (make-harmonia-audit
             :timestamp-ms (or (getf audit :timestamp-ms) (get-universal-time))
             :dissonance (float (or (getf audit :dissonance) 0.0d0)))
     :attachments (or (getf envelope :attachments) '())
     :transport (make-harmonia-transport
                 :kind (or (getf (getf envelope :transport) :kind)
                           (or (getf channel :kind) "unknown"))
                 :raw-address (or (getf (getf envelope :transport) :raw-address)
                                  (or (getf channel :address) "default"))
                 :raw-metadata (getf (getf envelope :transport) :raw-metadata))
     :taint :external)))

;;; --- Supervised action wrapper ---

(declaim (inline %supervised-action))
(defun %supervised-action (action-name thunk)
  "Execute THUNK with full error protection. Never propagates errors.
   Returns T if action completed without error, NIL if error was caught.
   Zero overhead on the happy path (handler-case is setjmp/longjmp in SBCL)."
  (handler-case
      (progn (funcall thunk) t)
    (serious-condition (c)
      ;; Error path only -- allocations here are fine since errors are rare
      (let ((msg (handler-case (princ-to-string c) (error () "<unprintable>"))))
        (%log :error "supervisor"
              "Action ~A failed: ~A" action-name (or msg "unknown error"))
        (handler-case

            (%push-error-ring
           (list :time (get-universal-time)
                 :action action-name
                 :error (or msg "unknown")
                 :cycle (and *runtime* (runtime-state-cycle *runtime*)

          (error () nil)))))
        (handler-case (record-runtime-error c) (error () nil))
        (sb-thread:with-mutex (*supervision-lock*)
          (incf *tick-error-count*))
        nil))))

;;; --- Prompt processing with restarts ---

(defun %process-prompt-safe (runtime prompt)
  (let ((log-prompt (%prompt-for-log prompt)))
    (restart-case
        (handler-bind
            ((error (lambda (c)
                      (%log :error "orchestrate" "~A" c)
                      (record-runtime-error c :prompt log-prompt)
                      (let ((r (find-restart 'continue-with-error)))
                        (when r (invoke-restart r))))))
          (let ((response (orchestrate-once prompt)))
            (handler-case
                (when (stringp prompt)
                  (maybe-self-rewrite prompt response))
              (error (e)
                (record-runtime-error e :prompt log-prompt)))
            response))
      (continue-with-error ()
        (runtime-log runtime :continue-with-error (list :prompt log-prompt))
        nil)
      (retry-prompt ()
        (%requeue-front runtime prompt)
        (runtime-log runtime :retry-prompt (list :prompt log-prompt))
        nil)
      (drop-prompt ()
        (runtime-log runtime :drop-prompt (list :prompt log-prompt))
        nil))))

;;; --- Tick: one supervised cycle (zero allocation on empty queue) ---

(defun tick (&key (runtime *runtime*))
  "Run one control-cycle iteration. Never crashes.
   Actions run inline -- no intermediate list allocated per tick."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))

  (setf (runtime-state-last-tick-at runtime) (get-universal-time))
  (incf (runtime-state-cycle runtime))

  ;; Run actions directly in order -- no planner list, no dispatch overhead
  ;; Phase 0: Poll tmux agents (triggers Rust-side detection + heartbeats)
  ;; Phase 1: Poll gateway + tailnet for inbound signals
  ;; Phase 1b: Drain actor mailbox, update states, detect stalls
  ;; Phase 1c: Auto-respond to interactive CLI prompts (permissions, confirmations)
  ;; Phase 1d: Supervision -- evaluate completed actors against frozen specs
  ;; Phase 2: Process one prompt from queue
  ;; Phase 2b: Deliver completed actor results
  ;; Phase 3-5: Memory, harmonic, chronicle, gateway flush
  (let ((ok0 (%tick-tmux-poll runtime))
        (ok1 (%tick-gateway-poll runtime))
        (ok1a (%tick-tailnet-poll runtime))
        (ok1b (%tick-actor-supervisor runtime))
        (ok1c (%tick-actor-interact runtime))
        (ok1d (%tick-supervision runtime))
        (ok2 (%tick-process-prompt runtime))
        (ok2b (%tick-actor-deliver runtime))
        (ok3 (%supervised-action "memory-heartbeat"
               (lambda () (memory-heartbeat :runtime runtime))))
        (ok3b (%supervised-action "recovery-heartbeat"
                (lambda ()
                  (when (fboundp '%tick-recovery-heartbeat)
                    (%tick-recovery-heartbeat)))))
        (ok4 (%supervised-action "harmonic-step"
               (lambda () (harmonic-state-step :runtime runtime))))
        (ok4b (%tick-chronicle-flush runtime))
        (ok5 (%tick-gateway-flush))
        (ok5b (%tick-tailnet-flush runtime)))
    (sb-thread:with-mutex (*supervision-lock*)
      (if (and ok0 ok1 ok1a ok1b ok1c ok1d ok2 ok2b ok3 ok4 ok4b ok5 ok5b)
          (setf *consecutive-tick-errors* 0)
          (incf *consecutive-tick-errors*))))

  (runtime-log runtime :tick (list :cycle (runtime-state-cycle runtime)
                                   :tools (hash-table-count (runtime-state-tools runtime))
                                   :queue (length (runtime-state-prompt-queue runtime))
                                   :errors *tick-error-count*))
  runtime)

;;; --- Actor-based runtime ---
;;;
;;; The actor system replaces the sequential tick loop with concurrent
;;; message-driven actors. Each subsystem owns its state and processes
;;; messages independently -- no subsystem blocks another.
;;;
;;; Architecture (inspired by cl-gserver/Sento):
;;;
;;;   Conductor       -- orchestrates prompts (LLM calls, tool dispatch)
;;;   Gateway         -- polls frontends, enqueues signals, flushes responses
;;;   Swarm           -- manages CLI subagents (tmux), actor lifecycle
;;;   Chronicle       -- batches and flushes event records
;;;   Harmonic        -- phase transitions, memory heartbeat
;;;   Signalograd     -- observation, feedback, projection
;;;
;;; Message flow:
;;;   Timer(:tick) -> Gateway -> Conductor -> Gateway (response)
;;;                -> Swarm   -> Conductor (actor results)
;;;                -> Harmonic, Chronicle (background)

(defvar *actor-system* nil
  "The Harmonia actor system. Created by run-actors, shut down by stop.")

(defun %make-receive-fn (name action-fn runtime)
  "Create a receive function that runs ACTION-FN for :tick messages.
   Wraps in supervision -- never crashes."
  (lambda (msg state)
    (case (actor-message-tag msg)
      (:tick
       (handler-case
           (funcall action-fn runtime)
         (serious-condition (c)
           (%log :error name "~A" (handler-case (princ-to-string c) (error () "<unprintable>")))
           (handler-case
               (%push-error-ring
              (list :time (get-universal-time) :action name
                    :error (handler-case (princ-to-string c) (error () "<unprintable>"))))
             (error () nil)))))
      (:stop nil))
    state))

(defun %make-conductor-receive (runtime)
  "The conductor actor: processes ONE prompt per :tick, delivers actor results,
   and flushes responses. This is the heart -- where signals become responses."
  (lambda (msg state)
    (case (actor-message-tag msg)
      (:tick
       (%supervised-action "process-prompt"
         (lambda () (%tick-process-prompt runtime)))
       (%supervised-action "actor-deliver"
         (lambda () (%tick-actor-deliver runtime)))
       (%supervised-action "gateway-flush"
         (lambda () (%tick-gateway-flush))))
      (:stop nil))
    state))

(defun run-actors (&key (runtime *runtime*) (sleep-seconds 1.0))
  "Start the actor system. Each subsystem becomes a concurrent actor
   driven by periodic :tick messages. Returns the actor system."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))
  (setf (runtime-state-running runtime) t)

  (let ((system (make-actor-system)))

    ;; -- Inbound actors (poll external sources) --

    (system-register system "gateway"
      (make-actor "gateway"
        (%make-receive-fn "gateway-poll"
          (lambda (rt)
            (%tick-gateway-poll rt)
            (%tick-tailnet-poll rt))
          runtime)))

    (system-register system "swarm"
      (make-actor "swarm"
        (%make-receive-fn "swarm"
          (lambda (rt)
            (%tick-tmux-poll rt)
            (%tick-actor-supervisor rt)
            (%tick-actor-interact rt)
            (%tick-supervision rt))
          runtime)))

    ;; -- Core actor (orchestration + response delivery) --

    (system-register system "conductor"
      (make-actor "conductor"
        (%make-conductor-receive runtime)))

    ;; -- Background actors (housekeeping, no urgency) --

    (system-register system "harmonic"
      (make-actor "harmonic"
        (%make-receive-fn "harmonic"
          (lambda (rt)
            (memory-heartbeat :runtime rt)
            (harmonic-state-step :runtime rt))
          runtime)))

    (system-register system "chronicle"
      (make-actor "chronicle"
        (%make-receive-fn "chronicle"
          (lambda (rt)
            (%tick-chronicle-flush rt)
            (%tick-tailnet-flush rt))
          runtime)))

    ;; -- Timers: drive each actor at its natural frequency --

    ;; Gateway + swarm poll every second (latency-sensitive)
    (system-add-timer system
      (start-timer (system-actor system "gateway") :tick sleep-seconds))
    (system-add-timer system
      (start-timer (system-actor system "swarm") :tick sleep-seconds))

    ;; Conductor ticks every second (prompt processing)
    (system-add-timer system
      (start-timer (system-actor system "conductor") :tick sleep-seconds))

    ;; Background actors tick every 5 seconds (no urgency)
    (system-add-timer system
      (start-timer (system-actor system "harmonic") :tick 5.0))
    (system-add-timer system
      (start-timer (system-actor system "chronicle") :tick 5.0))

    (setf *actor-system* system)
    (%log :info "actors" "Actor system started (5 actors, 5 timers).")
    system))

;;; --- Lifecycle ---

(defun stop (&optional (runtime *runtime*))
  "Request shutdown. Kills tmux agents, flushes chronicle, stops actor system."
  (when runtime
    ;; Kill all running tmux actors before shutdown
    (handler-case
        (maphash (lambda (id record)
                   (declare (ignore record))
                   (handler-case (tmux-kill id)
                     (error (e) (%log :warn "actor-supervisor" "tmux-kill ~D failed: ~A" id e))))
                 (runtime-state-actor-registry runtime))
      (error () nil))
    ;; Flush pending chronicle records
    (handler-case (%tick-chronicle-flush runtime) (error () nil))
    (setf (runtime-state-running runtime) nil)
    (runtime-log runtime :stop (list :cycle (runtime-state-cycle runtime))))
  (when *actor-system*
    (shutdown-system *actor-system*)
    (setf *actor-system* nil)
    (%log :info "actors" "Actor system shut down."))
  runtime)

(defun run-loop (&key (runtime *runtime*) (max-cycles nil) (sleep-seconds 1.0))
  "Run the Harmonia runtime.
   Uses the actor system by default -- concurrent, non-blocking subsystems.
   Falls back to sequential tick loop when max-cycles is set (test mode)."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))
  (setf (runtime-state-running runtime) t)

  (if max-cycles
      ;; Test mode: sequential tick loop (deterministic, for assertions)
      (progn
        (%log :info "loop" "Entering sequential loop (max-cycles=~D)." max-cycles)
        (loop while (and (runtime-state-running runtime)
                         (< (runtime-state-cycle runtime) max-cycles))
              do (handler-case (tick :runtime runtime)
                   (serious-condition (c)
                     (%log :error "supervisor" "Tick crash: ~A"
                           (handler-case (princ-to-string c) (error () "<unprintable>")))
                     (sb-thread:with-mutex (*supervision-lock*)
                       (incf *tick-error-count*))))
                 (sleep 0.05))
        (%log :info "loop" "Sequential loop exited after ~D cycles."
              (runtime-state-cycle runtime)))

      ;; Production mode: actor system (concurrent, non-blocking)
      (progn
        (%log :info "loop" "Entering supervised loop (sleep=~As)." sleep-seconds)
        (run-actors :runtime runtime :sleep-seconds sleep-seconds)
        ;; Block until stop is called (SIGTERM handler sets running=nil)
        (loop while (runtime-state-running runtime)
              do (sleep 1.0))
        (%log :info "loop" "Actor loop exited after ~D cycles (~D errors)."
              (runtime-state-cycle runtime) *tick-error-count*)))

  runtime)
