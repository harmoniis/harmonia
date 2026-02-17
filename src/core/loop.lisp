;;; loop.lisp — Core control loop.

(in-package :harmonia)

(defun tick (&key (runtime *runtime*))
  "Run one deterministic control-cycle iteration."
  (unless runtime
    (error "Runtime not initialized. Call HARMONIA:START first."))

  (setf (runtime-state-last-tick-at runtime) (get-universal-time))
  (incf (runtime-state-cycle runtime))

  (when (runtime-state-prompt-queue runtime)
    (let* ((prompt (first (runtime-state-prompt-queue runtime)))
           (response (orchestrate-once prompt)))
      (setf (runtime-state-prompt-queue runtime)
            (rest (runtime-state-prompt-queue runtime)))
      (maybe-self-rewrite prompt response)))

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
