;;; evolution.lisp — Port: self-improvement via artifact rollout (Phoenix) and source rewrite (Ouroboros).
;;;
;;; Two evolution modes:
;;;   :artifact-rollout (Phoenix) — binary swap via storage adapters (Phoenix is a supervisor binary)
;;;   :source-rewrite   (Ouroboros) — code self-modification via rust-forge + lineage
;;;
;;; NOTE: Ouroboros is not yet wired as an IPC component.
;;; Wrappers return sensible defaults until the Rust actor is connected.

(in-package :harmonia)

(defparameter *evolution-mode* :source-rewrite)
(defparameter *source-rewrite-enabled* t)
(defparameter *distributed-evolution-enabled* nil)
(defparameter *distributed-store-kind* "s3")
(defparameter *distributed-store-bucket* "")
(defparameter *distributed-store-prefix* "harmonia/evolution")

(declaim (ftype function evolution-snapshot-latest))

;;; --- Config helpers (pure Lisp, unchanged) ---

(defun %configure-evolution-runtime ()
  (let ((rewrite-raw (config-get-for "evolution" "source-rewrite-enabled")))
    (setf *source-rewrite-enabled*
          (if rewrite-raw
              (member (string-downcase rewrite-raw) '("1" "true" "yes" "on") :test #'string=)
              t)))
  (let ((dist-raw (config-get-for "evolution" "distributed-evolution-enabled")))
    (setf *distributed-evolution-enabled*
          (when dist-raw
            (member (string-downcase dist-raw) '("1" "true" "yes" "on") :test #'string=))))
  (setf *distributed-store-kind*
        (or (config-get-for "evolution" "distributed-store-kind") "s3"))
  (setf *distributed-store-bucket*
        (or (config-get-for "evolution" "distributed-store-bucket") ""))
  (setf *distributed-store-prefix*
        (or (config-get-for "evolution" "distributed-store-prefix")
            "harmonia/evolution"))
  (let ((mode-raw (string-downcase (or (config-get-for "evolution" "mode") ""))))
    (setf *evolution-mode*
          (cond
            ((string= mode-raw "artifact-rollout") :artifact-rollout)
            ((and (string= mode-raw "source-rewrite") *source-rewrite-enabled*)
             :source-rewrite)
            ((string= mode-raw "source-rewrite")
             :artifact-rollout)
            ((and *source-rewrite-enabled* (eq *evolution-mode* :source-rewrite))
             :source-rewrite)
            (t :artifact-rollout)))))

(defun %distributed-status ()
  (list :enabled *distributed-evolution-enabled*
        :store *distributed-store-kind*
        :bucket *distributed-store-bucket*
        :prefix *distributed-store-prefix*))

(defun %distributed-note (event payload)
  "Placeholder for publish/subscribe distributed evolution."
  (when *distributed-evolution-enabled*
    (runtime-log *runtime* :distributed-evolution
                 (list :event event :payload payload :store (%distributed-status)))))

;;; --- Port API ---

(defun init-evolution-port ()
  "Initialize evolution port. Ouroboros will be initialized when IPC component is wired."
  (%configure-evolution-runtime)
  (%log :info "evolution" "Evolution port initialized (IPC stub — ouroboros not yet wired)")
  (runtime-log *runtime* :evolution-init
               (list :mode *evolution-mode*
                     :source-rewrite-enabled *source-rewrite-enabled*
                     :distributed (%distributed-status)
                     :ouroboros :ipc-stub))
  t)

(defun evolution-mode ()
  "Query the current evolution mode."
  *evolution-mode*)

(defun evolution-set-mode (mode)
  "Set evolution mode to :ARTIFACT-ROLLOUT or :SOURCE-REWRITE."
  (unless (member mode '(:artifact-rollout :source-rewrite))
    (error "Invalid evolution mode: ~A (expected :artifact-rollout or :source-rewrite)" mode))
  (when (and (eq mode :source-rewrite) (not *source-rewrite-enabled*))
    (error "Source rewrite is disabled by policy/env (HARMONIA_SOURCE_REWRITE_ENABLED=0)."))
  (setf *evolution-mode* mode)
  (runtime-log *runtime* :evolution-mode-change (list :mode mode))
  mode)

(defun evolution-prepare ()
  "Dispatch to mode-specific preparation."
  (%log :warn "evolution" "evolution-prepare called on unwired IPC stub")
  (ecase *evolution-mode*
    (:artifact-rollout
     (list :mode :artifact-rollout :health "ipc-stub" :distributed (%distributed-status)))
    (:source-rewrite
     (list :mode :source-rewrite
           :health "ipc-stub"
           :last-crash "none"
           :distributed (%distributed-status)))))

(defun evolution-execute (&key component patch-body)
  "Dispatch to mode-specific execution."
  (%log :warn "evolution" "evolution-execute called on unwired IPC stub")
  (ecase *evolution-mode*
    (:artifact-rollout
     (let ((snapshot (ignore-errors
                       (evolution-snapshot-latest
                        :reason :artifact-rollout
                        :note "artifact rollout signaled"))))
       (if snapshot
           (progn
             (%distributed-note :artifact-rollout snapshot)
             (list :mode :artifact-rollout :status :signaled :snapshot snapshot))
           (list :mode :artifact-rollout :status :signaled :distributed (%distributed-status)))))
    (:source-rewrite
     (unless *source-rewrite-enabled*
       (error "source rewrite execution denied: source rewrite is disabled by policy"))
     (unless (and component patch-body)
       (error "evolution-execute in :source-rewrite mode requires :component and :patch-body"))
     ;; Ouroboros write-patch via IPC (will fail gracefully until wired)
     (let ((reply (ipc-call
                   (format nil "(:component \"ouroboros\" :op \"write-patch\" :component \"~A\" :patch-body \"~A\")"
                           (sexp-escape-lisp component) (sexp-escape-lisp patch-body)))))
       (when (ipc-reply-error-p reply)
         (error "ouroboros write-patch failed: ~A" (or reply "not wired")))
       (let ((snapshot (ignore-errors
                         (evolution-snapshot-latest
                          :reason :source-rewrite
                          :note component))))
         (if snapshot
             (progn
               (%distributed-note :source-rewrite snapshot)
               (list :mode :source-rewrite :status :patched :component component :snapshot snapshot))
             (list :mode :source-rewrite :status :patched :component component
                   :distributed (%distributed-status))))))))

(defun evolution-rollback ()
  "Undo last evolution step — records crash event for recovery tracking."
  (let ((reply (ipc-call
                (format nil "(:component \"ouroboros\" :op \"record-crash\" :component \"evolution\" :detail \"rollback requested\")"))))
    (when (ipc-reply-error-p reply)
      (%log :warn "evolution" "rollback record-crash failed: ~A" reply))
    (list :status :rolled-back :mode *evolution-mode*)))
