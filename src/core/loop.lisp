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

(defun %process-prompt-safe (runtime prompt)
  (restart-case
      (handler-bind
          ((error (lambda (c)
                    (record-runtime-error c :prompt prompt)
                    (let ((r (find-restart 'continue-with-error)))
                      (when r (invoke-restart r))))))
        (let ((response (orchestrate-once prompt)))
          (handler-case
              (maybe-self-rewrite prompt response)
            (error (e)
              (record-runtime-error e :prompt prompt)))
          response))
    (continue-with-error ()
      (runtime-log runtime :continue-with-error (list :prompt prompt))
      nil)
    (retry-prompt ()
      (%requeue-front runtime prompt)
      (runtime-log runtime :retry-prompt (list :prompt prompt))
      nil)
    (drop-prompt ()
      (runtime-log runtime :drop-prompt (list :prompt prompt))
      nil)))

(defun %reduce-tick-actions (runtime)
  "Pure-ish planner: derive a unidirectional action list for this cycle."
  (let ((actions '()))
    (let ((prompt (%queue-pop runtime)))
      (when prompt
        (push (list :process-prompt prompt) actions)))
    (push '(:memory-heartbeat) actions)
    (push '(:harmonic-step) actions)
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
