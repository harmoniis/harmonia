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
                :last-failure (get-universal-time)))))

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
                    (format nil "(:component \"~A\" :op \"reset\")"
                            (sexp-escape-lisp comp)))))
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

(defun %tick-recovery-heartbeat ()
  "Lightweight health check every 10 cycles. Restarts sick components."
  (when (and (boundp '*runtime*) *runtime*
             (zerop (mod (runtime-state-cycle *runtime*) 10)))
    (maphash (lambda (comp health)
               (let ((failures (or (getf health :failures) 0)))
                 (when (> failures 3)
                   (%log :info "heartbeat" "~A sick (~D failures). Restarting." comp failures)
                   (when (%restart-component comp)
                     (%health-record-success comp)))))
             *component-health*)))
