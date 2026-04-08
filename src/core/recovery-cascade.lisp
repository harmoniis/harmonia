;;; recovery-cascade.lisp — Component health tracking and heartbeat.
;;; Errors feed back into the eval loop (self-correcting).
;;; This file only tracks health state and provides the heartbeat.

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; HEALTH TRACKING — per-component, functional
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *component-health* (make-hash-table :test 'equal))

(defun %health-record-success (comp)
  (setf (gethash comp *component-health*)
        (list :failures 0 :last-success (get-universal-time))))

(defun %health-record-failure (comp)
  (let ((h (or (gethash comp *component-health*) '(:failures 0))))
    (setf (gethash comp *component-health*)
          (list :failures (1+ (or (getf h :failures) 0))
                :last-failure (get-universal-time)))
    ;; Evolutionary: record failure in memory field so patterns accumulate in basins.
    (handler-case
        (when (fboundp 'memory-put)
          (funcall 'memory-put :system
                 (format nil "Component failure: ~A (failures: ~D)"
                         comp (1+ (or (getf h :failures) 0)))
                 :depth 0
                 :tags (list :failure (intern (string-upcase comp) :keyword))))
      (error () nil))))

(defun %health-failures (comp)
  (or (getf (gethash comp *component-health*) :failures) 0))

;;; ═══════════════════════════════════════════════════════════════════════
;;; COMPONENT RESTART — generic, via IPC
;;; ═══════════════════════════════════════════════════════════════════════

(defun %restart-component (comp)
  "Restart a component via IPC reset."
  (%log :info "recovery" "Restarting: ~A" comp)
  (handler-case
      (let ((reply (ipc-call
                    (%sexp-to-ipc-string `(:component ,comp :op "reset")))))
        (and reply (ipc-reply-ok-p reply)))
    (error () nil)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; HONEST MESSAGES — never "internal error"
;;; ═══════════════════════════════════════════════════════════════════════

(defun %build-honest-error-message (comp operation)
  "Honest message when something fails. Generic, not hardcoded per component."
  (let ((failures (%health-failures comp)))
    (if (> failures 3)
        (format nil "I'm experiencing issues with ~A (~D consecutive failures). Self-diagnosing." comp failures)
        (format nil "Temporary issue with ~A. Using alternative approach." comp))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; HEARTBEAT — periodic health check + maintenance
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *last-heartbeat-cycle* 0)

(defun %tick-recovery-heartbeat ()
  "Heartbeat: health check + wake the agent via REPL.
   The agent decides what to do — dream, meditate, or nothing.
   No hardcoded logic. The field tells the LLM what's needed."
  (when (and (boundp '*runtime*) *runtime*)
    (let ((cycle (runtime-state-cycle *runtime*)))
      ;; Health check: every 10 cycles. This IS hardcoded — it's infrastructure, not intelligence.
      (when (zerop (mod cycle 10))
        (maphash (lambda (comp health)
                   (let ((failures (or (getf health :failures) 0)))
                     (when (> failures 3)
                       (%log :info "heartbeat" "~A sick (~D failures). Restarting." comp failures)
                       (when (%restart-component comp)
                         (%health-record-success comp)))))
                 *component-health*))
      ;; Wake the agent: every N cycles (DNA constraint), via REPL.
      ;; The LLM receives field context and decides: dream? meditate? nothing?
      (let ((interval (or (handler-case (dna-constraint :dream-cycle-interval) (error () nil)) 30)))
        (when (and (zerop (mod cycle interval))
                   (> (- cycle *last-heartbeat-cycle*) (- interval 5))
                   (fboundp '%orchestrate-repl))
          (%tick-heartbeat-wake cycle))))))

(defun %tick-heartbeat-wake (cycle)
  "Wake the agent via the ONE path: REPL. One word. Field provides the rest."
  (handler-case
      (progn
        (setf *last-heartbeat-cycle* cycle)
        (%log :info "heartbeat" "Wake cycle ~D" cycle)
        (funcall '%orchestrate-repl "HEARTBEAT"))
    (error (e)
      (%log :warn "heartbeat" "Wake failed: ~A" e))))
