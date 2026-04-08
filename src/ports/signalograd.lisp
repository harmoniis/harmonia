;;; signalograd.lisp — Port: Signalograd adaptive kernel via IPC.

(in-package :harmonia)

(defparameter *signalograd-ready* nil)

(defun signalograd-port-ready-p ()
  *signalograd-ready*)

(defun init-signalograd-port ()
  (let ((reply (ipc-signalograd-init)))
    (setf *signalograd-ready* (and reply (ipc-reply-ok-p reply)))
    (runtime-log *runtime* :signalograd-init
                 (list :status (if *signalograd-ready* :ok :failed)))
    *signalograd-ready*))

(defun signalograd-last-error ()
  "Signalograd errors are reported via IPC reply; this returns empty for compat."
  "")

(defun signalograd-observe (observation-sexp)
  (let ((reply (ipc-signalograd-observe observation-sexp)))
    (when (ipc-reply-error-p reply)
      (error "signalograd observe failed: ~A" reply))
    t))

(defun signalograd-reflect (observation)
  (signalograd-observe observation))

(defun signalograd-feedback (feedback-sexp)
  (let ((reply (ipc-signalograd-feedback feedback-sexp)))
    (when (ipc-reply-error-p reply)
      (error "signalograd feedback failed: ~A" reply))
    t))

(defun signalograd-checkpoint (path)
  (let ((reply (ipc-call
                (format nil "(:component \"signalograd\" :op \"checkpoint\" :path \"~A\")"
                        (sexp-escape-lisp path)))))
    (when (ipc-reply-error-p reply)
      (error "signalograd checkpoint failed: ~A" reply))
    t))

(defun signalograd-restore (path)
  (let ((reply (ipc-call
                (format nil "(:component \"signalograd\" :op \"restore\" :path \"~A\")"
                        (sexp-escape-lisp path)))))
    (when (ipc-reply-error-p reply)
      (error "signalograd restore failed: ~A" reply))
    t))

(defun signalograd-status ()
  (let ((text (ipc-signalograd-status)))
    (when text
      (let ((*read-eval* nil))
        (ignore-errors (read-from-string text))))))

(defun signalograd-snapshot ()
  (let ((text (ipc-signalograd-snapshot)))
    (when text
      (let ((*read-eval* nil))
        (ignore-errors (read-from-string text))))))

(defun signalograd-reset ()
  (let ((reply (ipc-signalograd-reset)))
    (when (ipc-reply-error-p reply)
      (error "signalograd reset failed: ~A" reply))
    t))
