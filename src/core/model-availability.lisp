;;; model-availability.lisp — self-healing model favorites.
;;;
;;; The favorites in config/model-policy.sexp (→ *model-profiles-all*) are a curated subset of the
;;; models OpenRouter actually serves. OpenRouter's live catalogue is synced into the metrics DB by
;;; the Rust provider-router; a favorite is "available" iff it appears in the most recent sync.
;;;
;;; At boot and on every harmonic :stabilize we SANITIZE: prune from the live *model-profiles* any
;;; favorite that is no longer available, drawing from the curated *model-profiles-all* (so a model
;;; that returns to availability is restored, not permanently lost). A deprecated model therefore
;;; self-removes instead of being selected and failing.
;;;
;;; Fail-safe, always: unknown availability (no sync yet / query error) NEVER prunes; a result that
;;; would empty the pool NEVER prunes. The agent can never be stranded with zero models.
;;; Pure functional + declarative; the only state touched is the model-policy module's own profiles.

(in-package :harmonia)

(defun model-available-ids ()
  "Currently-available model IDs from OpenRouter's live catalogue (latest sync), or NIL if unknown
(no sync yet / query failed). NIL means 'do not prune', never 'nothing is available'."
  (let* ((reply (ipc-call (%sexp-to-ipc-string
                           '(:component "provider-router" :op "available-models"))))
         (val (and reply (ipc-reply-ok-p reply) (ipc-extract-value reply))))
    (when (and (stringp val) (plusp (length val)))
      (let ((parsed (ignore-errors (let ((*read-eval* nil)) (read-from-string val)))))
        (when (and (listp parsed) parsed (every #'stringp parsed))
          parsed)))))

(defun %model-ids-hash (ids)
  (let ((h (make-hash-table :test #'equal)))
    (dolist (id ids h) (setf (gethash id h) t))))

(defun sanitize-model-favorites ()
  "Reconcile the live *model-profiles* against OpenRouter availability, from the curated
*model-profiles-all*. Returns the pruned profiles (NIL if none pruned). Fail-safe at two levels:
availability unknown → keep the full curated list; pruning would empty the pool → keep it too."
  (let ((available (model-available-ids)))
    (cond
      ;; Unknown availability — keep the curated favorites untouched (never prune on no data).
      ((null available)
       (setf *model-profiles* (copy-tree *model-profiles-all*))
       nil)
      (t
       (let* ((avail  (%model-ids-hash available))
              (kept   (remove-if-not (lambda (p) (gethash (getf p :id) avail)) *model-profiles-all*))
              (pruned (remove-if     (lambda (p) (gethash (getf p :id) avail)) *model-profiles-all*)))
         (cond
           ;; Pathological: not one favorite is available — keep all rather than strand the agent.
           ((null kept)
            (setf *model-profiles* (copy-tree *model-profiles-all*))
            (ignore-errors
             (%log :warn "model-availability"
                   (format nil "all ~A favorites absent from the live catalogue — keeping curated list (sync may be partial)"
                           (length *model-profiles-all*))))
            nil)
           (t
            (setf *model-profiles* kept)
            (when pruned
              (ignore-errors
               (%log :info "model-availability"
                     (format nil "pruned ~A unavailable favorite(s) [~{~A~^ ~}]; ~A kept of ~A live OpenRouter models"
                             (length pruned)
                             (mapcar (lambda (p) (getf p :id)) pruned)
                             (length kept) (length available)))))
            pruned)))))))

(defun model-availability-tick ()
  "Periodic self-heal hook — re-sanitize favorites against the latest sync. Safe + cheap; called
from the harmonic :stabilize cadence so deprecations are pruned without a restart."
  (ignore-errors (sanitize-model-favorites)))
