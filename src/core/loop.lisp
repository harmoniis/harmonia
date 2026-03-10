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

;;; ─── Queue operations ──────────────────────────────────────────────────

(defun %queue-pop (runtime)
  (let ((q (runtime-state-prompt-queue runtime)))
    (when q
      (setf (runtime-state-prompt-queue runtime) (rest q))
      (first q))))

(defun %requeue-front (runtime prompt)
  (setf (runtime-state-prompt-queue runtime)
        (cons prompt (runtime-state-prompt-queue runtime))))

;;; ─── Helpers ───────────────────────────────────────────────────────────

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

;;; ─── Supervised action wrapper ─────────────────────────────────────────

(declaim (inline %supervised-action))
(defun %supervised-action (action-name thunk)
  "Execute THUNK with full error protection. Never propagates errors.
   Returns T if action completed without error, NIL if error was caught.
   Zero overhead on the happy path (handler-case is setjmp/longjmp in SBCL)."
  (handler-case
      (progn (funcall thunk) t)
    (serious-condition (c)
      ;; Error path only — allocations here are fine since errors are rare
      (let ((msg (ignore-errors (princ-to-string c))))
        (%log :error "supervisor"
              "Action ~A failed: ~A" action-name (or msg "unknown error"))
        (ignore-errors
          (%push-error-ring
           (list :time (get-universal-time)
                 :action action-name
                 :error (or msg "unknown")
                 :cycle (and *runtime* (runtime-state-cycle *runtime*)))))
        (ignore-errors (record-runtime-error c))
        (incf *tick-error-count*)
        nil))))

;;; ─── Prompt processing with restarts ───────────────────────────────────

(defun %process-prompt-safe (runtime prompt)
  (let ((log-prompt (%prompt-for-log prompt)))
    (restart-case
        (handler-bind
            ((error (lambda (c)
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

;;; ─── Outbound queue ────────────────────────────────────────────────────

(defparameter *gateway-outbound-queue* '()
  "Outbound signals queued during a tick for gateway-flush.")

;;; ─── Tick action executors (inline, zero allocation) ─────────────────

(defun %tick-gateway-poll (runtime)
  "Poll gateway for inbound signals. Parses sexp batch and enqueues.
   Returns T on success (including idle polls with no signals)."
  (%supervised-action "gateway-poll"
    (lambda ()
      (let ((envelopes (gateway-poll)))
        (when (listp envelopes)
          (dolist (envelope envelopes)
            (let* ((signal-struct (%make-harmonia-signal-from-envelope envelope))
                   (dissonance (harmonia-signal-dissonance signal-struct)))
              (when (and (numberp dissonance) (> dissonance 0.0))
                (ignore-errors
                  (security-note-event
                   :frontend (or (harmonia-signal-channel-kind signal-struct) "unknown")
                   :injection-count (%dissonance->injection-count dissonance))))
              (setf (runtime-state-prompt-queue runtime)
                    (nconc (runtime-state-prompt-queue runtime)
                           (list signal-struct))))))))))

(defun %tick-actor-supervisor (runtime)
  "Drain unified actor mailbox and update actor records. Detect stalls.
   Dispatches by :kind field — handles :cli-agent, :llm-task, :gateway,
   :tailnet, :chronicle actor messages through the unified mailbox.
   Runs BEFORE process-prompt so newly completed actors are visible immediately.
   Returns T on success, NIL on error."
  (%supervised-action "actor-supervisor"
    (lambda ()
      (let* ((registry (runtime-state-actor-registry runtime))
             (messages (ignore-errors (actor-drain-mailbox)))
             (seen-ids (make-hash-table :test 'eql))
             (stall-threshold (%swarm-actor-stall-threshold)))
        ;; Process messages from unified actor mailbox
        (when (listp messages)
          (dolist (msg messages)
            (when (listp msg)
              (let* ((actor-id (getf msg :actor-id))
                     (actor-kind (getf msg :kind))
                     (raw-payload (getf msg :payload))
                     (payload-kind (and (listp raw-payload) (car raw-payload)))
                     ;; Strip type tag — raw-payload is (:completed :output "x" ...)
                     ;; so the plist properties start at (cdr raw-payload)
                     (payload (if (listp raw-payload) (cdr raw-payload) raw-payload)))
                ;; Dispatch by actor kind
                (cond
                  ;; CLI Agent / LLM Task messages — update Lisp actor registry
                  ((or (eq actor-kind :cli-agent) (eq actor-kind :llm-task))
                   (let ((record (gethash actor-id registry)))
                     (when record
                       (setf (gethash actor-id seen-ids) t)
                       (cond
                         ((eq payload-kind :progress-heartbeat)
                          (setf (actor-record-stall-ticks record) 0)
                          (setf (actor-record-last-heartbeat record) (get-universal-time)))
                         ((eq payload-kind :state-changed)
                          (let ((to (getf payload :to)))
                            (when (keywordp to)
                              (setf (actor-record-state record) to))))
                         ((eq payload-kind :completed)
                          (setf (actor-record-state record) :completed)
                          (setf (actor-record-result record) (or (getf payload :output) ""))
                          (setf (actor-record-duration-ms record) (or (getf payload :duration-ms) 0)))
                         ((eq payload-kind :failed)
                          (setf (actor-record-state record) :failed)
                          (setf (actor-record-error-text record) (or (getf payload :error) "unknown"))
                          (setf (actor-record-duration-ms record) (or (getf payload :duration-ms) 0)))))))
                  ;; Gateway inbound signals — already handled by %tick-gateway-poll
                  ;; (dual-path during transition, gateway messages are informational here)
                  ((eq actor-kind :gateway) nil)
                  ;; Tailnet mesh inbound — enqueue as internal signals
                  ((eq actor-kind :tailnet)
                   (when (eq payload-kind :mesh-inbound)
                     (ignore-errors
                       (%log :info "tailnet" "Mesh inbound from ~A: ~A"
                             (getf payload :from-node)
                             (getf payload :msg-type)))))
                  ;; Signalograd proposal — adaptive overlay for next cycle
                  ((eq actor-kind :signalograd)
                   (when (eq payload-kind :state-changed)
                     (let ((proposal (getf payload :to)))
                       (when (and (listp proposal)
                                  (eq (first proposal) :signalograd-proposal))
                         (ignore-errors
                           (signalograd-apply-proposal proposal :runtime runtime))))))
                  ;; Chronicle ack — informational
                  ((eq actor-kind :chronicle) nil))))))
        ;; Increment stall-ticks for actors that received NO messages this tick
        (maphash (lambda (id record)
                   (unless (gethash id seen-ids)
                     (when (eq (actor-record-state record) :running)
                       (incf (actor-record-stall-ticks record))
                       ;; Kill stalled actors
                       (when (>= (actor-record-stall-ticks record) stall-threshold)
                         (ignore-errors (tmux-kill id))
                         (setf (actor-record-state record) :failed)
                         (setf (actor-record-error-text record)
                               (format nil "actor stalled: ~D ticks with no output"
                                       (actor-record-stall-ticks record)))
                         (%log :warn "actor-supervisor"
                               "Killed stalled actor ~D (~A) after ~D ticks"
                               id (actor-record-model record)
                               (actor-record-stall-ticks record))))))
                 registry))
      t)))

(defun %tick-actor-deliver (runtime)
  "Deliver completed actor results to outbound queue and record outcomes.
   Runs AFTER process-prompt.
   Returns T on success, NIL on error."
  (%supervised-action "actor-deliver"
    (lambda ()
      (let ((registry (runtime-state-actor-registry runtime))
            (remaining '()))
        (dolist (actor-id (runtime-state-actor-pending runtime))
          (let ((record (gethash actor-id registry)))
            (cond
              ;; Completed: score, record, deliver
              ((and record (eq (actor-record-state record) :completed))
               (let* ((prompt (actor-record-prompt record))
                      (result (or (actor-record-result record) ""))
                      (trimmed (string-trim '(#\Space #\Newline #\Tab) result))
                      (visible (if (> (length trimmed) 0)
                                   (%presentation-sanitize-visible-text trimmed)
                                   "[actor completed with empty output]"))
                      (model (actor-record-model record))
                      (duration (or (actor-record-duration-ms record) 0))
                      (score (if (> (length trimmed) 0)
                                 (ignore-errors (harmonic-score prompt visible))
                                 0.0))
                      (cost (or (actor-record-cost-usd record) 0.0)))
                 ;; Record outcome
                 (ignore-errors
                   (model-policy-record-outcome
                    :model model :success t :latency-ms duration
                    :harmony-score (or score 0.0) :cost-usd cost))
                 ;; Record chronicle delegation
                 (ignore-errors
                   (chronicle-record-delegation
                    :task-hint "actor" :model model :backend "tmux-actor"
                    :reason "non-blocking CLI actor" :escalated nil
                    :cost-usd cost :latency-ms duration :success t
                    :tokens-in 0 :tokens-out 0))
                 (ignore-errors
                   (%presentation-record-response prompt
                                                  trimmed
                                                  :visible-response visible
                                                  :origin :actor
                                                  :model model
                                                  :score score
                                                  :harmony (list :mode :actor
                                                                 :llm-calls 1
                                                                 :tool-calls 0
                                                                 :datasource-count 1
                                                                 :intermediate-tokens 0)
                                                  :runtime runtime))
                 ;; Deliver to gateway if originating signal exists
                 (let ((signal (actor-record-originating-signal record)))
                   (when (harmonia-signal-p signal)
                     (push (list :frontend (harmonia-signal-frontend signal)
                                 :channel (harmonia-signal-sub-channel signal)
                                 :payload visible)
                           *gateway-outbound-queue*))))
               ;; Remove from registry
               (remhash actor-id registry))

              ;; Failed: log and remove
              ((and record (eq (actor-record-state record) :failed))
               (let ((model (actor-record-model record))
                     (error-text (or (actor-record-error-text record) "unknown"))
                     (prompt (actor-record-prompt record)))
                 (ignore-errors
                   (model-policy-record-outcome
                    :model model :success nil :latency-ms 0
                    :harmony-score 0.0 :cost-usd 0.0))
                 (ignore-errors (model-policy-mark-cli-cooloff (%swarm-cli-id model)))
                 (ignore-errors
                   (%presentation-record-response prompt
                                                  (format nil "[actor failed: ~A]" error-text)
                                                  :visible-response (format nil "[actor failed: ~A]"
                                                                            (%presentation-sanitize-visible-text error-text))
                                                  :origin :actor
                                                  :model model
                                                  :score 0.0
                                                  :harmony (list :mode :actor-failure)
                                                  :runtime runtime))
                 ;; Deliver error to gateway if originating signal exists
                 (let ((signal (actor-record-originating-signal record)))
                   (when (harmonia-signal-p signal)
                     (push (list :frontend (harmonia-signal-frontend signal)
                                 :channel (harmonia-signal-sub-channel signal)
                                 :payload (format nil "[actor failed: ~A]"
                                                  (%presentation-sanitize-visible-text error-text)))
                           *gateway-outbound-queue*))))
               (remhash actor-id registry))

              ;; Still running — keep in pending list
              (t (push actor-id remaining)))))
        (setf (runtime-state-actor-pending runtime) (nreverse remaining)))
      t)))

(defun %tick-process-prompt (runtime)
  "Pop one prompt and process it. Routes responses back to originating frontend.
   Handles :deferred responses from non-blocking actor spawns.
   Always sends a response for external signals — even on error — so frontends
   never hang waiting for a reply that will never come.
   Returns T on success or idle (no prompt), NIL only on actual error."
  (let ((prompt (%queue-pop runtime)))
    (if (null prompt)
        t  ; idle is not an error
        (%supervised-action "process-prompt"
          (lambda ()
            (let ((response (%process-prompt-safe runtime prompt)))
              (cond
                ;; Deferred: actor spawned, result delivered later by %tick-actor-deliver
                ((eq response :deferred)
                 nil)
                ;; Normal response for external signals
                ((harmonia-signal-p prompt)
                 (let* ((raw-payload (if response
                                         (if (stringp response) response
                                             (princ-to-string response))
                                         "[internal error — please try again]"))
                        (visible-payload (%presentation-sanitize-visible-text raw-payload)))
                   (when (null response)
                     (ignore-errors
                       (%presentation-record-response (harmonia-signal-payload prompt)
                                                      raw-payload
                                                      :visible-response visible-payload
                                                      :origin :system
                                                      :runtime runtime)))
                   (push (list :frontend (harmonia-signal-frontend prompt)
                               :channel (harmonia-signal-sub-channel prompt)
                               :payload visible-payload)
                         *gateway-outbound-queue*)))))
            t)))))

(defun %tick-tailnet-poll (runtime)
  "Poll tailnet for mesh inbound messages (via unified actor mailbox).
   Mesh messages arrive through the unified drain in %tick-actor-supervisor.
   This phase is a placeholder for any tailnet-specific polling beyond the mailbox."
  (declare (ignore runtime))
  (%supervised-action "tailnet-poll" (lambda () t)))

(defun %tick-tailnet-flush (runtime)
  "Flush queued outbound tailnet mesh messages.
   Currently a no-op placeholder — outbound mesh messages are sent inline."
  (declare (ignore runtime))
  (%supervised-action "tailnet-flush" (lambda () t)))

(defun %tick-chronicle-flush (runtime)
  "Flush batched chronicle recording requests in one operation.
   Collects all pending chronicle records accumulated during the tick and
   writes them in a single batch to reduce SQLite contention."
  (%supervised-action "chronicle-flush"
    (lambda ()
      (let ((pending (runtime-state-chronicle-pending runtime)))
        (when pending
          (setf (runtime-state-chronicle-pending runtime) '())
          (dolist (record pending)
            (handler-case
                (let ((type (getf record :type)))
                  (cond
                    ((string-equal type "harmonic")
                     (chronicle-record-harmonic (getf record :ctx)))
                    ((string-equal type "delegation")
                     (apply #'chronicle-record-delegation (getf record :args)))
                    ((string-equal type "memory")
                     (apply #'chronicle-record-memory-event (getf record :args)))))
              (error (e)
                (declare (ignore e))
                nil))))
        t))))

(defun %tick-gateway-flush ()
  "Drain outbound queue — send responses back through gateway.
   Processes both the Lisp-side outbound queue AND any OutboundSignal messages
   from the unified actor mailbox (posted by actors directly).
   Atomic swap: grab queue, clear it, iterate. No copy-list, no quadratic remove.
   Returns T on success or idle (empty queue), NIL only on actual error."
  (if (null *gateway-outbound-queue*)
      t  ; idle is not an error
      (%supervised-action "gateway-flush"
        (lambda ()
          (let ((batch *gateway-outbound-queue*))
            (setf *gateway-outbound-queue* '())
            (dolist (msg batch)
              (handler-case
                  (gateway-send (getf msg :frontend)
                                (getf msg :channel)
                                (%presentation-sanitize-visible-text (getf msg :payload)))
                (error (e)
                  (%log :warn "gateway-flush"
                        "Send to ~A/~A failed: ~A"
                        (getf msg :frontend) (getf msg :channel) e)
                  (ignore-errors (%record-lib-crash (getf msg :frontend) (princ-to-string e)))))))
          t))))

;;; ─── Tick: one supervised cycle (zero allocation on empty queue) ──────

(defun tick (&key (runtime *runtime*))
  "Run one control-cycle iteration. Never crashes.
   Actions run inline — no intermediate list allocated per tick."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))

  (setf (runtime-state-last-tick-at runtime) (get-universal-time))
  (incf (runtime-state-cycle runtime))

  ;; Run actions directly in order — no planner list, no dispatch overhead
  (let ((ok1 (%tick-gateway-poll runtime))
        (ok1a (%tick-tailnet-poll runtime))
        (ok1b (%tick-actor-supervisor runtime))
        (ok2 (%tick-process-prompt runtime))
        (ok2b (%tick-actor-deliver runtime))
        (ok3 (%supervised-action "memory-heartbeat"
               (lambda () (memory-heartbeat :runtime runtime))))
        (ok4 (%supervised-action "harmonic-step"
               (lambda () (harmonic-state-step :runtime runtime))))
        (ok4b (%tick-chronicle-flush runtime))
        (ok5 (%tick-gateway-flush))
        (ok5b (%tick-tailnet-flush runtime)))
    (if (and ok1 ok1a ok1b ok2 ok2b ok3 ok4 ok4b ok5 ok5b)
        (setf *consecutive-tick-errors* 0)
        (incf *consecutive-tick-errors*)))

  (runtime-log runtime :tick (list :cycle (runtime-state-cycle runtime)
                                   :tools (hash-table-count (runtime-state-tools runtime))
                                   :queue (length (runtime-state-prompt-queue runtime))
                                   :errors *tick-error-count*))
  runtime)

;;; ─── Lifecycle ─────────────────────────────────────────────────────────

(defun stop (&optional (runtime *runtime*))
  "Request loop shutdown."
  (when runtime
    (setf (runtime-state-running runtime) nil)
    (runtime-log runtime :stop (list :cycle (runtime-state-cycle runtime))))
  runtime)

(defun run-loop (&key (runtime *runtime*) (max-cycles nil) (sleep-seconds 1.0))
  "Run control loop until stop signal or max-cycles is reached.
   Erlang-style: the loop itself NEVER crashes. If a tick fails catastrophically,
   we log, cool down, and continue. The agent degrades gracefully but stays alive."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))
  (setf (runtime-state-running runtime) t)
  (%log :info "loop" "Entering supervised loop (sleep=~As)." sleep-seconds)
  (loop
    while (runtime-state-running runtime)
    do
    ;; The tick itself is wrapped — even if %supervised-action somehow
    ;; fails to catch something, this outer handler keeps the loop alive.
    (handler-case
        (tick :runtime runtime)
      (serious-condition (c)
        ;; This should never happen (tick actions are individually supervised),
        ;; but if it does, we survive.
        (%log :error "supervisor"
              "CRITICAL: tick-level crash at cycle ~D: ~A"
              (runtime-state-cycle runtime)
              (ignore-errors (princ-to-string c)))
        (ignore-errors
          (%push-error-ring
           (list :time (get-universal-time)
                 :action "tick"
                 :error (ignore-errors (princ-to-string c))
                 :cycle (runtime-state-cycle runtime))))
        (incf *tick-error-count*)
        (incf *consecutive-tick-errors*)))

    ;; Max-cycles check
    (when (and max-cycles
              (>= (runtime-state-cycle runtime) max-cycles))
      (stop runtime))

    ;; Adaptive sleep: if we're in an error storm, back off
    (let ((effective-sleep
            (if (>= *consecutive-tick-errors* *max-consecutive-errors-before-cooldown*)
                (progn
                  (when (= *consecutive-tick-errors* *max-consecutive-errors-before-cooldown*)
                    (%log :warn "supervisor"
                          "Entering cooldown: ~D consecutive error ticks. Backing off to ~As."
                          *consecutive-tick-errors*
                          (* sleep-seconds 5)))
                  (* sleep-seconds 5))  ; 5x slower during error storm
                sleep-seconds)))
      (sleep effective-sleep)))

  (%log :info "loop" "Loop exited after ~D cycles (~D errors total)."
        (runtime-state-cycle runtime) *tick-error-count*)
  runtime)
