;;; tick-gateway.lisp — Gateway poll and flush tick phases.

(in-package :harmonia)

;;; --- Outbound queue (thread-safe) ---

(defparameter *gateway-outbound-queue* '()
  "Outbound signals queued during a tick for gateway-flush.")
(defvar %outbound-lock (sb-thread:make-mutex :name "outbound-queue"))

(defun %outbound-push (msg)
  "Thread-safe push to the outbound gateway queue."
  (sb-thread:with-mutex (%outbound-lock)
    (push msg *gateway-outbound-queue*)))

(defun %outbound-drain ()
  "Thread-safe atomic drain of the outbound gateway queue."
  (sb-thread:with-mutex (%outbound-lock)
    (let ((items *gateway-outbound-queue*))
      (setf *gateway-outbound-queue* '())
      items)))

;;; --- Tick: gateway poll ---

(defun %tick-gateway-poll (runtime)
  "Poll gateway for inbound signals. Parses sexp batch and enqueues.
   The gateway intercepts /commands before they reach us -- only agent prompts
   arrive here. Also checks the gateway's pending-exit flag for /exit handling.
   Returns T on success (including idle polls with no signals)."
  (%supervised-action "gateway-poll"
    (lambda ()
      (let ((envelopes (gateway-poll)))
        (when envelopes
          (%log :info "tick" "Processing ~D envelopes" (length envelopes)))
        (when (listp envelopes)
          (dolist (envelope envelopes)
            (%log :info "tick" "Envelope: ~A" (write-to-string envelope :length 200))
            (let* ((signal-struct (%make-harmonia-signal-from-envelope envelope))
                   (dissonance (harmonia-signal-dissonance signal-struct))
                   (frontend (handler-case (harmonia-signal-frontend signal-struct) (error () nil)))
                   (channel-kind (handler-case (harmonia-signal-channel-kind signal-struct) (error () nil)))
                   (security-label (handler-case (harmonia-signal-security-label signal-struct) (error () nil)))
                   (payload-length (handler-case (length (harmonia-signal-payload signal-struct)) (error () nil))))
              ;; Only emit trace when a signal ACTUALLY arrives (not on empty polls)
              (when (%trace-level-p :minimal)
                (trace-event "signal-received" :chain
                             :metadata (list :frontend frontend
                                             :channel-kind channel-kind
                                             :security-label security-label
                                             :dissonance (or dissonance 0.0)
                                             :payload-length (or payload-length 0))))
              (when (and (numberp dissonance) (> dissonance 0.0))
                (handler-case

                    (security-note-event
                   :frontend (or channel-kind "unknown")
                   :injection-count (%dissonance->injection-count dissonance)

                  (error () nil))))
              (%log :info "tick" "Enqueuing signal from ~A payload-len=~D"
                    (or frontend "?") (or payload-length 0))
              (%queue-push runtime signal-struct)))))
      ;; Check if the gateway intercepted /exit
      (when (= (%gateway-pending-exit) 1)
        (stop runtime)))))

;;; --- Tick: gateway flush ---

(defun %tick-gateway-flush ()
  "Drain outbound queue -- send responses back through gateway.
   Processes both the Lisp-side outbound queue AND any OutboundSignal messages
   from the unified actor mailbox (posted by actors directly).
   Atomic swap: grab queue, clear it, iterate. No copy-list, no quadratic remove.
   Returns T on success or idle (empty queue), NIL only on actual error."
  (let ((batch (%outbound-drain)))
    (if (null batch)
        t
        (%supervised-action "gateway-flush"
          (lambda ()
            (dolist (msg batch)
              (handler-case
                  (let ((payload (or (getf msg :payload) "")))
                    (gateway-send (getf msg :frontend)
                                  (getf msg :channel)
                                  payload))
                (error (e)
                  (%log :warn "gateway-flush"
                        "Send to ~A/~A failed: ~A"
                        (getf msg :frontend) (getf msg :channel) e)
                  (handler-case

                      (%record-lib-crash (getf msg :frontend)
                                       (princ-to-string e)

                    (error () nil))))))
            t)))))
