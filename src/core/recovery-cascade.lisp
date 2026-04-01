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
    (ignore-errors
      (when (fboundp 'memory-put)
        (funcall 'memory-put :system
                 (format nil "Component failure: ~A (failures: ~D)"
                         comp (1+ (or (getf h :failures) 0)))
                 :depth 0
                 :tags (list :failure (intern (string-upcase comp) :keyword)))))))

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

(defparameter *last-dream-cycle* 0)

(defun %tick-recovery-heartbeat ()
  "Lightweight health check every 10 cycles. Restarts sick components.
   Every 30 cycles: trigger dreaming if system is idle and stable."
  (when (and (boundp '*runtime*) *runtime*)
    (let ((cycle (runtime-state-cycle *runtime*)))
      ;; Health check: every 10 cycles.
      (when (zerop (mod cycle 10))
        (maphash (lambda (comp health)
                   (let ((failures (or (getf health :failures) 0)))
                     (when (> failures 3)
                       (%log :info "heartbeat" "~A sick (~D failures). Restarting." comp failures)
                       (when (%restart-component comp)
                         (%health-record-success comp)))))
                 *component-health*))
      ;; Dreaming: interval and idle threshold from DNA constraints.
      (let ((interval (or (ignore-errors (dna-constraint :dream-cycle-interval)) 30))
            (idle-min (or (ignore-errors (dna-constraint :dream-idle-ticks)) 5)))
        (when (and (zerop (mod cycle interval))
                   (> (- cycle *last-dream-cycle*) (- interval idle-min))
                   (fboundp 'memory-field-port-ready-p)
                   (funcall 'memory-field-port-ready-p))
          (%tick-dream cycle))))))

(defun %tick-dream (cycle)
  "Field self-maintenance — the agent dreams. Prunes stale entries, crystallizes structural ones."
  (handler-case
      (let ((dream-report (funcall 'memory-field-dream)))
        (when dream-report
          (let ((results (funcall '%apply-dream-results dream-report)))
            (setf *last-dream-cycle* cycle)
            (%log :info "dream" "Dreaming complete: ~A" results)
            ;; Record dream event for Chronicle.
            (ignore-errors
              (when (fboundp 'ouroboros-record-crash)
                (funcall 'ouroboros-record-crash "dreaming"
                         (format nil "pruned=~D crystallized=~D"
                                 (or (getf results :pruned) 0)
                                 (or (getf results :crystallized) 0))))))))
    (error (e)
      (%log :warn "dream" "Dreaming failed: ~A" e))))
