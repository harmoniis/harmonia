;;; tui-test-agent.lisp — long-running SBCL agent for TUI integration testing.
;;;
;;; Loads boot, starts harmonia, and pumps the actor system at native cadence
;;; until killed. Prompts arrive via the TUI socket → gateway → conductor →
;;; REPL → response back through the gateway to the TUI client.

(in-package :cl-user)
(load #P"src/core/boot.lisp")

(format *error-output* "[tui-test-agent] starting harmonia~%")
(force-output *error-output*)

;; Start with auto-loop so the actor system spins continuously and processes
;; prompts as they arrive.
(harmonia:start :run-loop t)

;; harmonia:start with :run-loop t spawns the actor system; we just sleep
;; until killed. The agent processes prompts on its own threads.
(loop
  (sleep 5)
  (handler-case
      (when (and (boundp 'harmonia:*runtime*) harmonia:*runtime*)
        (let ((q (length (harmonia::runtime-state-prompt-queue harmonia:*runtime*))))
          (when (> q 0)
            (format *error-output* "[tui-test-agent] queue depth ~D~%" q)
            (force-output *error-output*))))
    (error () nil)))
