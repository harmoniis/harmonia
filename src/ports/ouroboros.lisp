;;; ouroboros.lisp — Port: Self-healing crash ledger and patch writing via IPC.
;;; HOMOICONIC: all IPC commands built as lists via %sexp-to-ipc-string.

(in-package :harmonia)

(defun init-ouroboros-port ()
  (let ((reply (handler-case
     (ipc-call (%sexp-to-ipc-string
                            '(:component "ouroboros" :op "healthcheck")
   (error () nil))))))
    (if (and reply (ipc-reply-ok-p reply))
        (progn (%log :info "ouroboros" "Ouroboros actor ready") t)
        (progn (%log :warn "ouroboros" "Ouroboros actor not available") nil))))

(defun ouroboros-record-crash (component detail)
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "ouroboros" :op "record-crash"
                             :component-name ,component :detail ,detail)))))
    (ipc-reply-ok-p reply)))

(defun ouroboros-last-crash ()
  (let* ((reply (ipc-call (%sexp-to-ipc-string
                            '(:component "ouroboros" :op "last-crash"))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun ouroboros-history (&optional (limit 20))
  (let* ((reply (ipc-call (%sexp-to-ipc-string
                            `(:component "ouroboros" :op "history" :limit ,limit))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun ouroboros-write-patch (component patch-body)
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "ouroboros" :op "write-patch"
                             :component-name ,component :patch-body ,patch-body)))))
    (ipc-reply-ok-p reply)))
