;;; state.lisp — Runtime state container.

(in-package :harmonia)

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
                            (lorenz-z 0.0))))
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
  lorenz-z)

(defun runtime-log (runtime tag payload)
  (push (list :time (get-universal-time) :tag tag :payload payload)
        (runtime-state-events runtime))
  runtime)
