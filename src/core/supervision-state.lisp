;;; supervision-state.lisp — Shared supervisor counters loaded before readers.

(in-package :harmonia)

(defparameter *tick-error-count* 0
  "Total errors caught by the supervisor across all ticks.")

(defparameter *consecutive-tick-errors* 0
  "Consecutive ticks that had at least one error. Reset on clean tick.")

(defparameter *max-consecutive-errors-before-cooldown* 10
  "After this many consecutive error ticks, enter cooldown (longer sleep).")

(defvar *supervision-lock* (sb-thread:make-mutex :name "supervision-counters"))

(defmacro with-supervision-lock (() &body body)
  `(sb-thread:with-mutex (*supervision-lock*) ,@body))
