;;; ouroboros.lisp — Port: Self-healing crash ledger and patch writing via IPC.
;;;
;;; The Ouroboros actor records failures, writes patches, and maintains
;;; the crash history that feeds evolutionary circuit breakers.

(in-package :harmonia)

(defun init-ouroboros-port ()
  "Initialize ouroboros port. Verifies the actor responds."
  (let ((reply (ignore-errors
                 (ipc-call "(:component \"ouroboros\" :op \"healthcheck\")"))))
    (if (and reply (ipc-reply-ok-p reply))
        (progn (%log :info "ouroboros" "Ouroboros actor ready")
               t)
        (progn (%log :warn "ouroboros" "Ouroboros actor not available")
               nil))))

;;; ─── Public API ────────────────────────────────────────────────────

(defun ouroboros-record-crash (component detail)
  "Record a crash event to the Ouroboros recovery ledger."
  (let ((reply (ipc-call
                (format nil "(:component \"ouroboros\" :op \"record-crash\" :component-name \"~A\" :detail \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp detail)))))
    (ipc-reply-ok-p reply)))

(defun ouroboros-last-crash ()
  "Return the most recent crash event."
  (let* ((reply (ipc-call "(:component \"ouroboros\" :op \"last-crash\")"))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (ignore-errors (read-from-string reply)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun ouroboros-history (&optional (limit 20))
  "Return crash history (last LIMIT events)."
  (let* ((reply (ipc-call
                 (format nil "(:component \"ouroboros\" :op \"history\" :limit ~D)" limit)))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (ignore-errors (read-from-string reply)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun ouroboros-write-patch (component patch-body)
  "Write a source patch via Ouroboros. Returns T on success."
  (let ((reply (ipc-call
                (format nil "(:component \"ouroboros\" :op \"write-patch\" :component-name \"~A\" :patch-body \"~A\")"
                        (sexp-escape-lisp component) (sexp-escape-lisp patch-body)))))
    (ipc-reply-ok-p reply)))
