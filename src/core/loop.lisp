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

;;; ─── Supervision counters ──────────────────────────────────────────────

(defparameter *tick-error-count* 0
  "Total errors caught by the supervisor across all ticks.")
(defparameter *consecutive-tick-errors* 0
  "Consecutive ticks that had at least one error. Reset on clean tick.")
(defparameter *max-consecutive-errors-before-cooldown* 10
  "After this many consecutive error ticks, enter cooldown (longer sleep).")

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

(defun %metadata-string-value (metadata key)
  "Extract KEY from metadata s-expression string as :key \"value\"."
  (let* ((meta (or metadata ""))
         (needle (format nil ":~A \"" key))
         (start (search needle meta :test #'char-equal)))
    (when start
      (let* ((from (+ start (length needle)))
             (to (position #\" meta :start from)))
        (and to (subseq meta from to))))))

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
      (let ((raw (gateway-poll)))
        (when (and raw (> (length raw) 0) (not (string= raw "nil")))
          (let* ((*read-eval* nil)
                 (signals (ignore-errors (read-from-string raw))))
            (when (listp signals)
              (dolist (sig signals)
                (let* ((payload (getf sig :payload))
                       (channel (getf sig :channel))
                       (frontend (getf channel :frontend))
                       (sub-channel (getf channel :sub-channel))
                       (security (getf sig :security))
                       (capabilities (getf sig :capabilities))
                       (metadata (getf sig :metadata))
                       (metadata-str (if metadata (princ-to-string metadata) nil))
                       (security-override (%metadata-string-value metadata-str "security"))
                       (origin-fp (%metadata-string-value metadata-str "origin-fp"))
                       (effective-security (or security-override security))
                       (security-kw (%security-keyword-from-string effective-security))
                       (dissonance (or (getf sig :dissonance) 0.0d0)))
                  (when payload
                    (let ((signal-struct
                            (make-harmonia-signal
                             :id (or (getf sig :id) 0)
                             :frontend (or frontend "unknown")
                             :sub-channel (or sub-channel "default")
                             :security-label security-kw
                             :payload payload
                             :capabilities (if capabilities (princ-to-string capabilities) nil)
                             :metadata metadata-str
                             :timestamp-ms (or (getf sig :timestamp) (get-universal-time))
                             :taint :external
                             :dissonance (float dissonance)
                             :origin-fp origin-fp)))
                      (when (and (numberp dissonance) (> dissonance 0.0))
                        (ignore-errors
                          (security-note-event
                           :frontend (or frontend "unknown")
                           :injection-count (%dissonance->injection-count dissonance))))
                      ;; nconc avoids copying the entire queue on append
                      (setf (runtime-state-prompt-queue runtime)
                            (nconc (runtime-state-prompt-queue runtime)
                                   (list signal-struct))))))))))))))

(defun %tick-process-prompt (runtime)
  "Pop one prompt and process it. Routes responses back to originating frontend.
   Always sends a response for external signals — even on error — so frontends
   never hang waiting for a reply that will never come.
   Returns T on success or idle (no prompt), NIL only on actual error."
  (let ((prompt (%queue-pop runtime)))
    (if (null prompt)
        t  ; idle is not an error
        (%supervised-action "process-prompt"
          (lambda ()
            (let ((response (%process-prompt-safe runtime prompt)))
              (when (harmonia-signal-p prompt)
                (push (list :frontend (harmonia-signal-frontend prompt)
                            :channel (harmonia-signal-sub-channel prompt)
                            :payload (if response
                                         (if (stringp response) response
                                             (princ-to-string response))
                                         "[internal error — please try again]"))
                      *gateway-outbound-queue*)))
            t)))))

(defun %tick-gateway-flush ()
  "Drain outbound queue — send responses back through gateway.
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
                  (gateway-send (getf msg :frontend) (getf msg :channel) (getf msg :payload))
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
        (ok2 (%tick-process-prompt runtime))
        (ok3 (%supervised-action "memory-heartbeat"
               (lambda () (memory-heartbeat :runtime runtime))))
        (ok4 (%supervised-action "harmonic-step"
               (lambda () (harmonic-state-step :runtime runtime))))
        (ok5 (%tick-gateway-flush)))
    (if (and ok1 ok2 ok3 ok4 ok5)
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
