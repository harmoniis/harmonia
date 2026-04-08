;;; tick-process.lisp — Core prompt processing tick phase and related phases.

(in-package :harmonia)

;;; --- Tick: process prompt ---

(defun %tick-process-prompt (runtime)
  "Pop one prompt and process it. Routes responses back to originating frontend.
   Handles :deferred responses from non-blocking actor spawns.
   Always sends a response for external signals -- even on error -- so frontends
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
                                         (if (fboundp '%build-honest-error-message)
                                             (%build-honest-error-message "orchestration" "process-prompt")
                                             "I encountered a temporary issue. Let me try a different approach.")))
                        (visible-payload (%presentation-sanitize-visible-text raw-payload)))
                   (when (null response)
                     (handler-case

                         (%presentation-record-response (harmonia-signal-payload prompt)
                                                      raw-payload
                                                      :visible-response visible-payload
                                                      :origin :system
                                                      :runtime runtime)

                       (error () nil)))
                   (%outbound-push
                    (list :frontend (harmonia-signal-frontend prompt)
                          :channel (harmonia-signal-sub-channel prompt)
                          :payload visible-payload))))))
            t)))))

;;; --- Tick: tailnet poll/flush ---

(defun %tick-tailnet-poll (runtime)
  "Poll tailnet for mesh inbound messages (via unified actor mailbox).
   Mesh messages arrive through the unified drain in %tick-actor-supervisor.
   This phase is a placeholder for any tailnet-specific polling beyond the mailbox."
  (declare (ignore runtime))
  (%supervised-action "tailnet-poll" (lambda () t)))

(defun %tick-tailnet-flush (runtime)
  "Flush queued outbound tailnet mesh messages.
   Currently a no-op placeholder -- outbound mesh messages are sent inline."
  (declare (ignore runtime))
  (%supervised-action "tailnet-flush" (lambda () t)))

;;; --- Tick: chronicle flush ---

(defun %tick-chronicle-flush (runtime)
  "Flush batched chronicle recording requests in one operation.
   Collects all pending chronicle records accumulated during the tick and
   writes them in a single batch to reduce SQLite contention."
  (%supervised-action "chronicle-flush"
    (lambda ()
      (let ((pending (runtime-state-chronicle-pending runtime)))
        (when pending
          (when (%trace-level-p :verbose)
            (trace-event "chronicle-flush" :tool
                         :metadata (list :records-count (length pending))))
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
                     (apply #'chronicle-record-memory-event (getf record :args)))
                    ((string-equal type "supervision-spec")
                     (%chronicle-flush-supervision-spec (getf record :args)))
                    ((string-equal type "supervision-verdict")
                     (%chronicle-flush-supervision-verdict (getf record :args)))))
              (error (e)
                (declare (ignore e))
                nil))))
        t))))
