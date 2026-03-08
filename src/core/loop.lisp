;;; loop.lisp — Core control loop.

(in-package :harmonia)

(defun %queue-pop (runtime)
  (let ((q (runtime-state-prompt-queue runtime)))
    (when q
      (setf (runtime-state-prompt-queue runtime) (rest q))
      (first q))))

(defun %requeue-front (runtime prompt)
  (setf (runtime-state-prompt-queue runtime)
        (cons prompt (runtime-state-prompt-queue runtime))))

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

(defparameter *gateway-outbound-queue* '()
  "Outbound signals queued during a tick for gateway-flush.")

(defun %reduce-tick-actions (runtime)
  "Pure-ish planner: derive a unidirectional action list for this cycle."
  (let ((actions '()))
    (push '(:gateway-poll) actions)
    (let ((prompt (%queue-pop runtime)))
      (when prompt
        (push (list :process-prompt prompt) actions)))
    (push '(:memory-heartbeat) actions)
    (push '(:harmonic-step) actions)
    (push '(:gateway-flush) actions)
    (nreverse actions)))

(defun %run-tick-action (runtime action)
  "Side-effect executor: handles exactly one effect action."
  (case (first action)
    (:process-prompt
     (%process-prompt-safe runtime (second action)))
    (:memory-heartbeat
     (handler-case (memory-heartbeat :runtime runtime)
       (error (e) (record-runtime-error e))))
    (:harmonic-step
     (handler-case (harmonic-state-step :runtime runtime)
       (error (e) (record-runtime-error e))))
    (:gateway-poll
     (handler-case
         (let ((raw (gateway-poll)))
           (when (and raw (> (length raw) 0))
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
	                         (setf (runtime-state-prompt-queue runtime)
	                               (append (runtime-state-prompt-queue runtime)
	                                       (list signal-struct)))))))))))
       (error (e) (record-runtime-error e))))
    (:gateway-flush
     (handler-case
         (dolist (msg (copy-list *gateway-outbound-queue*))
           (gateway-send (getf msg :frontend) (getf msg :channel) (getf msg :payload))
           (setf *gateway-outbound-queue*
                 (remove msg *gateway-outbound-queue* :test #'eq)))
       (error (e) (record-runtime-error e))))
    (t
     (runtime-log runtime :unknown-tick-action action))))

(defun tick (&key (runtime *runtime*))
  "Run one deterministic control-cycle iteration with action->effect flow."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))

  (setf (runtime-state-last-tick-at runtime) (get-universal-time))
  (incf (runtime-state-cycle runtime))

  (dolist (action (%reduce-tick-actions runtime))
    (%run-tick-action runtime action))

  (runtime-log runtime :tick (list :cycle (runtime-state-cycle runtime)
                                   :tools (hash-table-count (runtime-state-tools runtime))
                                   :queue (length (runtime-state-prompt-queue runtime))))
  runtime)

(defun stop (&optional (runtime *runtime*))
  "Request loop shutdown."
  (when runtime
    (setf (runtime-state-running runtime) nil)
    (runtime-log runtime :stop (list :cycle (runtime-state-cycle runtime))))
  runtime)

(defun run-loop (&key (runtime *runtime*) (max-cycles nil) (sleep-seconds 1.0))
  "Run control loop until stop signal or max-cycles is reached."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))
  (setf (runtime-state-running runtime) t)
  (loop
    while (runtime-state-running runtime)
    do (tick :runtime runtime)
       (when (and max-cycles
                  (>= (runtime-state-cycle runtime) max-cycles))
         (stop runtime))
       (sleep sleep-seconds))
  runtime)
