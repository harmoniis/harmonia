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
                            (active-model "qwen/qwen3-coder:free"))))
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
  active-model)

(defun runtime-log (runtime tag payload)
  (push (list :time (get-universal-time) :tag tag :payload payload)
        (runtime-state-events runtime))
  runtime)
