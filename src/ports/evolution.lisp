;;; evolution.lisp — Port: binary evolution via artifact rollout.
;;;
;;; The agent evolves its binary at runtime, not its source code.
;;; Evolution = REPL s-expressions that modify behavior at runtime.
;;; Binary rollout via S3 or local storage for version management.
;;; No git, no source rewrite, no patch files.

(in-package :harmonia)

(defparameter *evolution-mode* :artifact-rollout)
(defparameter *distributed-evolution-enabled* nil)
(defparameter *distributed-store-kind* "s3")
(defparameter *distributed-store-bucket* "")
(defparameter *distributed-store-prefix* "harmonia/evolution")

(declaim (ftype function evolution-snapshot-latest))

(defun %configure-evolution-runtime ()
  (let ((dist-raw (config-get-for "evolution" "distributed-evolution-enabled")))
    (setf *distributed-evolution-enabled*
          (when dist-raw
            (member (string-downcase dist-raw) '("1" "true" "yes" "on") :test #'string=))))
  (setf *distributed-store-kind*
        (or (config-get-for "evolution" "distributed-store-kind") "s3"))
  (setf *distributed-store-bucket*
        (or (config-get-for "evolution" "distributed-store-bucket") ""))
  (setf *distributed-store-prefix*
        (or (config-get-for "evolution" "distributed-store-prefix") "harmonia/evolution")))

(defun %distributed-status ()
  (list :enabled *distributed-evolution-enabled*
        :store *distributed-store-kind*
        :bucket *distributed-store-bucket*
        :prefix *distributed-store-prefix*))

(defun %distributed-note (event payload)
  (when *distributed-evolution-enabled*
    (runtime-log *runtime* :distributed-evolution
                 (list :event event :payload payload :store (%distributed-status)))))

(defun init-evolution-port ()
  "Initialize evolution port. Binary rollout only."
  (%configure-evolution-runtime)
  (%log :info "evolution" "Evolution port initialized (artifact-rollout)")
  t)

(defun evolution-mode () *evolution-mode*)

(defun evolution-prepare ()
  (list :mode :artifact-rollout :health :ready :distributed (%distributed-status)))

(defun evolution-execute (&key component patch-body)
  "Execute binary evolution — snapshot and optionally distribute."
  (declare (ignore component patch-body))
  (let ((snapshot (handler-case
     (evolution-snapshot-latest
                     :reason :artifact-rollout
                     :note "binary evolution")
   (error () nil))))
    (when snapshot (%distributed-note :artifact-rollout snapshot))
    (list :mode :artifact-rollout :status :signaled :snapshot snapshot)))

(defun evolution-rollback ()
  (list :status :rolled-back :mode :artifact-rollout))
