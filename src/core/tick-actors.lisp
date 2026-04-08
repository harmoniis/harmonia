;;; tick-actors.lisp — Actor supervisor, interaction, and delivery tick phases.

(in-package :harmonia)

;;; --- Tick: tmux poll ---

(defun %tick-tmux-poll (runtime)
  "Poll all active tmux CLI agents via Rust swarm-poll.
   This triggers terminal capture, state detection, and posts heartbeat/state
   messages to the unified actor mailbox. Must run BEFORE actor-supervisor
   so messages are available for drain.
   Returns T on success, NIL on error."
  (declare (ignore runtime))
  (%supervised-action "tmux-poll"
    (lambda ()
      ;; tmux-swarm-poll calls Rust controller::swarm_poll() which:
      ;; 1. Captures each tmux agent's terminal output
      ;; 2. Runs detection (processing/permission/confirmation/selection/completed)
      ;; 3. Posts ProgressHeartbeat + StateChanged messages to unified mailbox
      ;; NOTE: No trace event here -- this fires every tick (1/sec).
      ;; Actor lifecycle events are emitted by %tick-actor-supervisor on state changes.
      (handler-case (tmux-swarm-poll) (error () nil))
      t)))

;;; --- Actor state helpers ---

(defun %actor-state-interactive-p (state)
  "Return T if STATE represents an interactive prompt (waiting for human/conductor input).
   Interactive actors must NOT accumulate stall-ticks -- they're waiting for us, not stalled."
  (or (eq state :waiting-for-permission)
      (eq state :waiting-for-confirmation)
      (eq state :waiting-for-selection)
      (eq state :waiting-for-input)
      (eq state :onboarding)
      (eq state :plan-mode)))

(defun %parse-actor-state-changed (to)
  "Parse a state-changed :to value from the actor mailbox.
   The Rust side sends keyword for simple states (:completed, :processing, etc.)
   and sexp lists for interactive states ((:waiting-for-permission :tool ...)).
   Returns (values state-keyword detail-plist)."
  (cond
    ((keywordp to) (values to nil))
    ((and (listp to) (keywordp (car to)))
     (values (car to) (cdr to)))
    (t (values :running nil))))

;;; --- Tick: actor supervisor ---

(defun %tick-actor-supervisor (runtime)
  "Drain unified actor mailbox and update actor records. Detect stalls.
   Dispatches by :kind field -- handles :cli-agent, :llm-task, :gateway,
   :tailnet, :chronicle actor messages through the unified mailbox.
   Runs BEFORE process-prompt so newly completed actors are visible immediately.
   Returns T on success, NIL on error."
  (%supervised-action "actor-supervisor"
    (lambda ()
      (let* ((registry (runtime-state-actor-registry runtime))
             (messages (handler-case (actor-drain-mailbox) (error () nil)))
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
                     ;; Strip type tag -- raw-payload is (:completed :output "x" ...)
                     ;; so the plist properties start at (cdr raw-payload)
                     (payload (if (listp raw-payload) (cdr raw-payload) raw-payload)))
                ;; Dispatch by actor kind
                (cond
                  ;; CLI Agent / LLM Task messages -- update Lisp actor registry
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
                            (multiple-value-bind (state-kw detail)
                                (%parse-actor-state-changed to)
                              (declare (ignore detail))
                              ;; Only trace on ACTUAL state transitions, not redundant updates
                              (let ((from-state (actor-record-state record)))
                                (when (and (not (eq from-state state-kw))
                                           (%trace-level-p :standard))
                                  (trace-event "actor-state-change" :agent
                                               :metadata (list :actor-id actor-id
                                                               :model (actor-record-model record)
                                                               :from-state from-state
                                                               :to-state state-kw
                                                               :stall-ticks (actor-record-stall-ticks record)))))
                              (setf (actor-record-state record) state-kw)
                              ;; Interactive states reset stall counter -- actor is alive, waiting for us
                              (when (%actor-state-interactive-p state-kw)
                                (setf (actor-record-stall-ticks record) 0)))))
                         ((eq payload-kind :completed)
                          (setf (actor-record-state record) :completed)
                          (setf (actor-record-result record) (or (getf payload :output) ""))
                          (setf (actor-record-duration-ms record) (or (getf payload :duration-ms) 0)))
                         ((eq payload-kind :failed)
                          (setf (actor-record-state record) :failed)
                          (setf (actor-record-error-text record) (or (getf payload :error) "unknown"))
                          (setf (actor-record-duration-ms record) (or (getf payload :duration-ms) 0)))))))
                  ;; Gateway inbound signals -- already handled by %tick-gateway-poll
                  ;; (dual-path during transition, gateway messages are informational here)
                  ((eq actor-kind :gateway) nil)
                  ;; Tailnet mesh inbound -- enqueue as internal signals
                  ((eq actor-kind :tailnet)
                   (when (eq payload-kind :mesh-inbound)
                     (handler-case

                         (%log :info "tailnet" "Mesh inbound from ~A: ~A"
                             (getf payload :from-node)
                             (getf payload :msg-type)

                       (error () nil)))))
                  ;; Signalograd proposal -- adaptive overlay for next cycle
                  ((eq actor-kind :signalograd)
                   (when (eq payload-kind :state-changed)
                     (let ((proposal (getf payload :to)))
                       (when (and (listp proposal)
                                  (eq (first proposal) :signalograd-proposal))
                         (handler-case

                             (signalograd-apply-proposal proposal :runtime runtime)

                           (error () nil))))))
                  ;; Chronicle ack -- informational
                  ((eq actor-kind :chronicle) nil))))))
        ;; Increment stall-ticks for actors that received NO messages this tick
        ;; Skip actors in interactive states -- they're waiting for conductor, not stalled
        (maphash (lambda (id record)
                   (unless (gethash id seen-ids)
                     (let ((state (actor-record-state record)))
                       (when (and (eq state :running)
                                  (not (%actor-state-interactive-p state)))
                         (incf (actor-record-stall-ticks record))
                         ;; Kill stalled actors
                         (when (>= (actor-record-stall-ticks record) stall-threshold)
                           (when (%trace-level-p :standard)
                           (trace-event "actor-stalled" :agent
                                        :metadata (list :actor-id id
                                                        :model (actor-record-model record)
                                                        :stall-ticks (actor-record-stall-ticks record)
                                                        :threshold stall-threshold)))
                           (handler-case (tmux-kill id)
                           (error (e) (%log :warn "actor-supervisor" "tmux-kill ~D failed: ~A" id e)))
                           (setf (actor-record-state record) :failed)
                           (setf (actor-record-error-text record)
                                 (format nil "actor stalled: ~D ticks with no output"
                                         (actor-record-stall-ticks record)))
                           (%log :warn "actor-supervisor"
                                 "Killed stalled actor ~D (~A) after ~D ticks"
                                 id (actor-record-model record)
                                 (actor-record-stall-ticks record)))))))
                 registry))
      t)))

;;; --- Tick: actor interact ---

(defun %tick-actor-interact (runtime)
  "Handle interactive CLI agent prompts: permissions, confirmations, selections.
   Scans actor registry for actors in interactive states and auto-responds.
   Policy: auto-approve permissions, auto-confirm yes, select first option.
   Runs AFTER actor-supervisor so states are up-to-date.
   Returns T on success, NIL on error."
  (%supervised-action "actor-interact"
    (lambda ()
      (let ((registry (runtime-state-actor-registry runtime)))
        (maphash
         (lambda (id record)
           (let ((state (actor-record-state record)))
             (handler-case
                 (cond
                   ;; Permission prompt -> auto-approve
                   ;; Claude Code launched with --dangerously-skip-permissions should
                   ;; not normally reach here, but some prompts still leak through.
                   ((eq state :waiting-for-permission)
                    (%log :info "actor-interact"
                          "Auto-approving permission for actor ~D (~A)"
                          id (actor-record-model record))
                    (tmux-approve id)
                    (setf (actor-record-state record) :running)
                    (setf (actor-record-stall-ticks record) 0))
                   ;; Confirmation prompt -> auto-confirm yes
                   ((eq state :waiting-for-confirmation)
                    (%log :info "actor-interact"
                          "Auto-confirming for actor ~D (~A)"
                          id (actor-record-model record))
                    (tmux-confirm-yes id)
                    (setf (actor-record-state record) :running)
                    (setf (actor-record-stall-ticks record) 0))
                   ;; Selection menu -> select first option
                   ((eq state :waiting-for-selection)
                    (%log :info "actor-interact"
                          "Auto-selecting first option for actor ~D (~A)"
                          id (actor-record-model record))
                    (tmux-select-option id 0)
                    (setf (actor-record-state record) :running)
                    (setf (actor-record-stall-ticks record) 0))
                   ;; Onboarding/survey/first-run -> dismiss with Enter
                   ((eq state :onboarding)
                    (%log :info "actor-interact"
                          "Auto-dismissing onboarding for actor ~D (~A)"
                          id (actor-record-model record))
                    (tmux-send-key id "Enter")
                    (setf (actor-record-state record) :running)
                    (setf (actor-record-stall-ticks record) 0))
                   ;; Plan mode -> auto-accept plan
                   ((eq state :plan-mode)
                    (%log :info "actor-interact"
                          "Auto-accepting plan for actor ~D (~A)"
                          id (actor-record-model record))
                    (tmux-confirm-yes id)
                    (setf (actor-record-state record) :running)
                    (setf (actor-record-stall-ticks record) 0))
                   ;; WaitingForInput in non-interactive mode likely means task finished
                   ;; but completion wasn't detected. Don't interfere -- let stall detection
                   ;; handle it if output truly stops.
                   )
               (error (e)
                 (%log :warn "actor-interact"
                       "Failed to interact with actor ~D: ~A" id e)))))
         registry))
      t)))

;;; --- Tick: actor deliver ---

(defparameter *swarm-group-timeout-ticks* 10
  "Max ticks to wait after first completion in a group before delivering best available.")

(defun %tick-actor-deliver (runtime)
  "Deliver completed actor results to outbound queue and record outcomes.
   Group-aware: actors sharing a swarm-group-id are delivered as one (best by harmony score).
   Singletons (nil group-id) deliver immediately (legacy behavior).
   Runs AFTER process-prompt.
   Returns T on success, NIL on error."
  (%supervised-action "actor-deliver"
    (lambda ()
      (let ((registry (runtime-state-actor-registry runtime))
            (remaining '())
            (groups (make-hash-table :test 'eql)))
        ;; Pass 0: poll Rust engine for completed tasks and update records
        (dolist (actor-id (runtime-state-actor-pending runtime))
          (let ((record (gethash actor-id registry)))
            (when (and record (eq (actor-record-state record) :running))
              (handler-case

                  (let ((result (parallel-task-result actor-id)

                (error () nil)))
                  (when (and result (stringp result) (> (length result) 0)
                             (not (search "pending" result))
                             (not (search "running" result)))
                    (let ((parsed (%swarm-parse-task-result result (actor-record-model record))))
                      (when parsed
                        (setf (actor-record-state record)
                              (if (getf parsed :success) :completed :failed))
                        (setf (actor-record-result record) (getf parsed :text))
                        (setf (actor-record-duration-ms record)
                              (or (getf parsed :latency-ms) 0))
                        (setf (actor-record-cost-usd record)
                              (or (getf parsed :cost-usd) 0.0))))))))))
        ;; Pass 1: partition pending actors into groups and singletons
        (dolist (actor-id (runtime-state-actor-pending runtime))
          (let ((record (gethash actor-id registry)))
            (when record
              (let ((gid (actor-record-swarm-group-id record)))
                (if gid
                    (push actor-id (gethash gid groups))
                    ;; Singleton: deliver immediately (legacy behavior)
                    (%deliver-singleton-actor runtime registry actor-id record remaining))))))
        ;; Pass 2: process groups -- deliver best when all terminal or timed out
        (maphash
         (lambda (gid actor-ids)
           (let ((all-terminal t)
                 (any-completed nil)
                 (first-completion-at nil)
                 (completed '())
                 (failed '()))
             ;; Classify group members
             (dolist (aid actor-ids)
               (let ((rec (gethash aid registry)))
                 (when rec
                   (case (actor-record-state rec)
                     (:completed
                      (push (cons aid rec) completed)
                      (setf any-completed t)
                      (let ((spawned (actor-record-spawned-at rec)))
                        (when (or (null first-completion-at)
                                  (< spawned first-completion-at))
                          (setf first-completion-at spawned))))
                     (:failed
                      (push (cons aid rec) failed))
                     (otherwise
                      (setf all-terminal nil))))))
             (let* ((seconds-since-first
                      (if first-completion-at
                          (- (get-universal-time) first-completion-at)
                          0))
                    (timed-out (>= seconds-since-first (* *swarm-group-timeout-ticks* 1)))
                    (should-deliver (or all-terminal (and any-completed timed-out))))
               (if should-deliver
                   ;; Score all completed, deliver ONLY the best
                   (let ((best-aid nil)
                         (best-rec nil)
                         (best-score -1.0))
                     (dolist (pair completed)
                       (let* ((aid (car pair))
                              (rec (cdr pair))
                              (prompt (actor-record-prompt rec))
                              (result (or (actor-record-result rec) ""))
                              (trimmed (string-trim '(#\Space #\Newline #\Tab) result))
                              (score (if (> (length trimmed) 0)
                                         (or (handler-case (harmonic-score prompt trimmed) (error () nil)) 0.0)
                                         0.0)))
                         (when (> score best-score)
                           (setf best-score score
                                 best-aid aid
                                 best-rec rec))))
                     ;; Deliver best
                     (when best-rec
                       (%deliver-completed-actor runtime registry best-aid best-rec))
                     ;; Record outcomes for non-best completed actors and remove
                     (dolist (pair completed)
                       (unless (eql (car pair) best-aid)
                         (%record-actor-outcome (cdr pair))
                         (remhash (car pair) registry)))
                     ;; Record failed actors and remove
                     (dolist (pair failed)
                       (%record-failed-actor runtime registry (car pair) (cdr pair)))
                     ;; Kill any still-running actors in the group
                     (dolist (aid actor-ids)
                       (let ((rec (gethash aid registry)))
                         (when (and rec
                                    (not (member (actor-record-state rec) '(:completed :failed))))
                           (handler-case (tmux-kill aid) (error () nil))
                           (remhash aid registry)))))
                   ;; Not ready yet -- keep all in pending
                   (dolist (aid actor-ids)
                     (push aid remaining))))))
         groups)
        (setf (runtime-state-actor-pending runtime) (nreverse remaining)))
      t)))

;;; --- Actor delivery helpers ---

(defun %deliver-singleton-actor (runtime registry actor-id record remaining)
  "Deliver a singleton actor (no group-id) immediately. Legacy behavior."
  (cond
    ((eq (actor-record-state record) :completed)
     (%deliver-completed-actor runtime registry actor-id record))
    ((eq (actor-record-state record) :failed)
     (%record-failed-actor runtime registry actor-id record))
    (t (push actor-id remaining))))

(defun %deliver-completed-actor (runtime registry actor-id record)
  "Score, record, and deliver a completed actor result to the gateway."
  (let* ((prompt (actor-record-prompt record))
         (result (or (actor-record-result record) ""))
         (trimmed (string-trim '(#\Space #\Newline #\Tab) result))
         (visible (if (> (length trimmed) 0)
                      (%presentation-sanitize-visible-text trimmed)
                      "[actor completed with empty output]"))
         (model (actor-record-model record))
         (duration (or (actor-record-duration-ms record) 0))
         (score (if (> (length trimmed) 0)
                    (handler-case (harmonic-score prompt visible) (error () nil))
                    0.0))
         (cost (or (actor-record-cost-usd record) 0.0)))
    (when (%trace-level-p :standard)
      (trace-event "actor-completed" :agent
                   :metadata (list :actor-id actor-id
                                   :model model
                                   :score (or score 0.0)
                                   :cost-usd cost
                                   :duration-ms duration
                                   :supervision-grade (actor-record-supervision-grade record)
                                   :result-length (length trimmed))))
    ;; Record outcome
    (handler-case

        (model-policy-record-outcome
       :model model :success t :latency-ms duration
       :harmony-score (or score 0.0) :cost-usd cost)

      (error () nil))
    ;; Record chronicle delegation (includes supervision if available)
    (let ((sv-grade (actor-record-supervision-grade record))
          (sv-confidence (or (actor-record-supervision-confidence record) 0.0)))
      (handler-case

          (chronicle-record-delegation
         :task-hint "actor" :model model :backend "tmux-actor"
         :reason (if sv-grade
                     (format nil "non-blocking CLI actor [supervision: ~A ~,2F]"
                             (string-downcase (symbol-name sv-grade)

        (error () nil)) sv-confidence)
                     "non-blocking CLI actor")
         :escalated nil
         :cost-usd cost :latency-ms duration :success t
         :tokens-in 0 :tokens-out 0)))
    (handler-case

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
                                     :runtime runtime)

      (error () nil))
    ;; Deliver to gateway if originating signal exists
    (let ((signal (actor-record-originating-signal record)))
      (when (harmonia-signal-p signal)
        (%outbound-push
         (list :frontend (harmonia-signal-frontend signal)
               :channel (harmonia-signal-sub-channel signal)
               :payload visible)))))
  ;; Remove from registry
  (remhash actor-id registry))

(defun %record-actor-outcome (record)
  "Record outcome metrics for a non-delivered actor (non-best in a group)."
  (let ((model (actor-record-model record))
        (duration (or (actor-record-duration-ms record) 0))
        (cost (or (actor-record-cost-usd record) 0.0)))
    (handler-case

        (model-policy-record-outcome
       :model model :success t :latency-ms duration
       :harmony-score 0.0 :cost-usd cost)

      (error () nil))))

(defun %record-failed-actor (runtime registry actor-id record)
  "Record and deliver a failed actor result."
  (let* ((model (actor-record-model record))
         (raw-error (or (actor-record-error-text record) "unknown"))
         ;; Clip error text to 80 chars -- prevents partial LLM output from leaking
         (error-text (if (> (length raw-error) 80)
                         (concatenate 'string (subseq raw-error 0 80) "...")
                         raw-error))
         (prompt (actor-record-prompt record)))
    (when (%trace-level-p :minimal)
      (trace-event "actor-failed" :agent
                   :metadata (list :actor-id actor-id
                                   :model model
                                   :error-text error-text
                                   :duration-ms (or (actor-record-duration-ms record) 0))))
    (handler-case

        (model-policy-record-outcome
       :model model :success nil :latency-ms 0
       :harmony-score 0.0 :cost-usd 0.0)

      (error () nil))
    (handler-case (model-policy-mark-cli-cooloff (%swarm-cli-id model)) (error () nil))
    (handler-case

        (%presentation-record-response prompt
                                     (format nil "[actor failed: ~A]" error-text)
                                     :visible-response (format nil "[actor failed: ~A]"
                                                               (%presentation-sanitize-visible-text error-text)

      (error () nil))
                                     :origin :actor
                                     :model model
                                     :score 0.0
                                     :harmony (list :mode :actor-failure)
                                     :runtime runtime))
    ;; Deliver error to gateway if originating signal exists
    (let ((signal (actor-record-originating-signal record)))
      (when (harmonia-signal-p signal)
        (when (%trace-level-p :standard)
          (trace-event "actor-error-delivered" :agent
                       :metadata (list :actor-id actor-id
                                       :model model
                                       :error-text error-text
                                       :frontend (harmonia-signal-frontend signal))))
        (%outbound-push
         (list :frontend (harmonia-signal-frontend signal)
               :channel (harmonia-signal-sub-channel signal)
               :payload (format nil "[actor failed: ~A]"
                                (%presentation-sanitize-visible-text error-text)))))))
  (remhash actor-id registry))
