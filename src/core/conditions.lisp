;;; conditions.lisp — Condition/restart primitives for non-crashing evolution.

(in-package :harmonia)

(define-condition harmonia-runtime-error (error)
  ((phase :initarg :phase :reader hre-phase)
   (detail :initarg :detail :reader hre-detail)
   (payload :initarg :payload :initform nil :reader hre-payload))
  (:report (lambda (c s)
             (format s "Harmonia runtime error phase=~A detail=~A"
                     (hre-phase c) (hre-detail c)))))

(define-condition harmonia-backend-error (harmonia-runtime-error) ())
(define-condition harmonia-evolution-error (harmonia-runtime-error) ())
(define-condition harmonia-compiler-error (harmonia-runtime-error) ())

(declaim (ftype function harmonic-matrix-log-event))

(defun %string-contains-ci (hay needle)
  (and hay needle
       (search (string-downcase needle) (string-downcase hay) :test #'char=)))

(defun classify-runtime-error (condition)
  (let ((msg (princ-to-string condition)))
    (cond
      ((or (%string-contains-ci msg "cargo")
           (%string-contains-ci msg "rustc")
           (%string-contains-ci msg "compiler")
           (%string-contains-ci msg "compile"))
       :compiler)
      ((or (%string-contains-ci msg "openrouter")
           (%string-contains-ci msg "provider-router")
           (%string-contains-ci msg "backend")
           (%string-contains-ci msg "provider"))
       :backend)
      (t :evolution))))

(defun record-runtime-error (condition &key prompt)
  (let* ((kind (classify-runtime-error condition))
         (msg (princ-to-string condition))
         (tags (list :error kind)))
    (memory-put :daily
                (list :error msg
                      :kind kind
                      :prompt prompt
                      :cycle (and *runtime* (runtime-state-cycle *runtime*)))
                :depth 0
                :tags tags)
    (runtime-log *runtime* :runtime-error (list :kind kind :message msg :prompt prompt))
    (handler-case
        (harmonic-matrix-log-event "runtime" "error" (string-downcase (string kind))
                                   (or prompt "")
                                   nil
                                   msg)
      (error () nil))
    kind))
