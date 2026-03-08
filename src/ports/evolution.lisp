;;; evolution.lisp — Port: self-improvement via artifact rollout (Phoenix) and source rewrite (Ouroboros).
;;;
;;; Two evolution modes:
;;;   :artifact-rollout (Phoenix) — binary swap via storage adapters (Phoenix is a supervisor binary)
;;;   :source-rewrite   (Ouroboros) — code self-modification via rust-forge + lineage
;;;
;;; Phoenix is a standalone supervisor binary; Ouroboros exposes CFFI for crash tracking + patching.

(in-package :harmonia)

(defparameter *evolution-mode* :source-rewrite)
(defparameter *ouroboros-lib* nil)
(defparameter *source-rewrite-enabled* t)
(defparameter *distributed-evolution-enabled* nil)
(defparameter *distributed-store-kind* "s3")
(defparameter *distributed-store-bucket* "")
(defparameter *distributed-store-prefix* "harmonia/evolution")

(declaim (ftype function evolution-snapshot-latest))

;;; --- Ouroboros CFFI ---

(cffi:defcfun ("harmonia_ouroboros_version" %ouroboros-version) :string)
(cffi:defcfun ("harmonia_ouroboros_healthcheck" %ouroboros-healthcheck) :int)
(cffi:defcfun ("harmonia_ouroboros_record_crash" %ouroboros-record-crash) :int
  (component :string)
  (detail :string))
(cffi:defcfun ("harmonia_ouroboros_last_crash" %ouroboros-last-crash) :pointer)
(cffi:defcfun ("harmonia_ouroboros_history" %ouroboros-history) :pointer
  (limit :int))
(cffi:defcfun ("harmonia_ouroboros_write_patch" %ouroboros-write-patch) :int
  (component :string)
  (patch-body :string))
(cffi:defcfun ("harmonia_ouroboros_last_error" %ouroboros-last-error) :pointer)
(cffi:defcfun ("harmonia_ouroboros_health" %ouroboros-health) :pointer)
(cffi:defcfun ("harmonia_ouroboros_free_string" %ouroboros-free-string) :void
  (ptr :pointer))

;;; --- Helpers ---

(defun %ouroboros-read-string (ptr op)
  (if (cffi:null-pointer-p ptr)
      (error "ouroboros ~A failed: ~A" op (%ouroboros-error-string))
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (%ouroboros-free-string ptr))))

(defun %ouroboros-error-string ()
  (let ((ptr (%ouroboros-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%ouroboros-free-string ptr)))))

(defun %env-true-p (name &optional (default nil))
  (let ((raw (sb-ext:posix-getenv name)))
    (if raw
        (member (string-downcase raw) '("1" "true" "yes" "on") :test #'string=)
        default)))

(defun %configure-evolution-runtime ()
  (setf *source-rewrite-enabled* (%env-true-p "HARMONIA_SOURCE_REWRITE_ENABLED" t))
  (setf *distributed-evolution-enabled*
        (%env-true-p "HARMONIA_DISTRIBUTED_EVOLUTION_ENABLED" nil))
  (setf *distributed-store-kind*
        (or (sb-ext:posix-getenv "HARMONIA_DISTRIBUTED_STORE_KIND") "s3"))
  (setf *distributed-store-bucket*
        (or (sb-ext:posix-getenv "HARMONIA_DISTRIBUTED_STORE_BUCKET") ""))
  (setf *distributed-store-prefix*
        (or (sb-ext:posix-getenv "HARMONIA_DISTRIBUTED_STORE_PREFIX")
            "harmonia/evolution"))
  (let ((mode-raw (string-downcase (or (sb-ext:posix-getenv "HARMONIA_EVOLUTION_MODE") ""))))
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
  "Placeholder for publish/subscribe distributed evolution.
   TODO: implement S3-sync pub/sub algorithm (proposal digest, quorum, rollback)."
  (when *distributed-evolution-enabled*
    (runtime-log *runtime* :distributed-evolution
                 (list :event event :payload payload :store (%distributed-status)))))

;;; --- Port API ---

(defun init-evolution-port ()
  "Load the ouroboros dylib. Phoenix is a standalone binary and not loaded here."
  (%configure-evolution-runtime)
  (ensure-cffi)
  (setf *ouroboros-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_ouroboros.dylib")))
  (runtime-log *runtime* :evolution-init
               (list :mode *evolution-mode*
                     :source-rewrite-enabled *source-rewrite-enabled*
                     :distributed (%distributed-status)
                     :ouroboros :loaded))
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
  (ecase *evolution-mode*
    (:artifact-rollout
     ;; Phoenix mode: check health via ouroboros crash history
     (let ((health (%ouroboros-read-string (%ouroboros-health) "health")))
       (list :mode :artifact-rollout :health health :distributed (%distributed-status))))
    (:source-rewrite
     ;; Ouroboros mode: check for pending patches and crash state
     (let ((health (%ouroboros-read-string (%ouroboros-health) "health"))
           (last-crash (%ouroboros-read-string (%ouroboros-last-crash) "last-crash")))
       (list :mode :source-rewrite
             :health health
             :last-crash last-crash
             :distributed (%distributed-status))))))

(defun evolution-execute (&key component patch-body)
  "Dispatch to mode-specific execution."
  (ecase *evolution-mode*
    (:artifact-rollout
     ;; Phoenix mode: signal readiness (actual swap done by phoenix supervisor)
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
     ;; Ouroboros mode: write a patch
     (unless *source-rewrite-enabled*
       (error "source rewrite execution denied: source rewrite is disabled by policy"))
     (unless (and component patch-body)
       (error "evolution-execute in :source-rewrite mode requires :component and :patch-body"))
     (let ((rc (%ouroboros-write-patch component patch-body)))
       (unless (zerop rc)
         (error "ouroboros write-patch failed: ~A" (%ouroboros-error-string)))
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
  (let ((rc (%ouroboros-record-crash "evolution" "rollback requested")))
    (unless (zerop rc)
      (error "ouroboros record-crash failed: ~A" (%ouroboros-error-string)))
    (list :status :rolled-back :mode *evolution-mode*)))
