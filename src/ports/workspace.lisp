;;; workspace.lisp — Port: Workspace file operations via IPC actor.
;;;
;;; The agent's hands for reading the world. All file operations go through
;;; the Rust workspace actor (parallel, async, path-sandboxed).
;;; The LLM controls these through REPL only — tools are actors, not Lisp code.

(in-package :harmonia)

(defun init-workspace-port ()
  "Initialize workspace port. Verifies the actor responds."
  (let ((reply (ignore-errors
                 (ipc-call "(:component \"workspace\" :op \"healthcheck\")"))))
    (if (and reply (ipc-reply-ok-p reply))
        (progn (%log :info "workspace" "Workspace actor ready")
               t)
        (progn (%log :warn "workspace" "Workspace actor not available")
               nil))))

;;; ─── Public API ────────────────────────────────────────────────────

(defun workspace-read-file (path &key (offset 0) (limit 200))
  "Read file lines via workspace actor. Returns result string."
  (let* ((sexp (format nil "(:component \"workspace\" :op \"read-file\" :path \"~A\" :offset ~D :limit ~D)"
                       (sexp-escape-lisp path) offset limit))
         (reply (ipc-call sexp))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (ignore-errors (read-from-string reply)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun workspace-grep (pattern &optional (path ".") &key (limit 30))
  "Grep workspace for pattern via actor. Returns result string."
  (let* ((sexp (format nil "(:component \"workspace\" :op \"grep\" :pattern \"~A\" :path \"~A\" :limit ~D)"
                       (sexp-escape-lisp pattern) (sexp-escape-lisp path) limit))
         (reply (ipc-call sexp))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (ignore-errors (read-from-string reply)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun workspace-list-files (&optional (path ".") &key (pattern "") (limit 50))
  "List files in directory via workspace actor."
  (let* ((sexp (format nil "(:component \"workspace\" :op \"list-files\" :path \"~A\" :pattern \"~A\" :limit ~D)"
                       (sexp-escape-lisp path) (sexp-escape-lisp pattern) limit))
         (reply (ipc-call sexp)))
    (if (and reply (ipc-reply-ok-p reply))
        reply
        "(no results)")))

(defun workspace-file-exists-p (path)
  "Check if file exists via workspace actor."
  (let* ((sexp (format nil "(:component \"workspace\" :op \"file-exists\" :path \"~A\")"
                       (sexp-escape-lisp path)))
         (reply (ipc-call sexp))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (ignore-errors (read-from-string reply)))))
    (when (listp parsed) (eq t (getf (cdr parsed) :exists)))))

(defun workspace-file-info (path)
  "Get file size/lines/type via workspace actor."
  (let* ((sexp (format nil "(:component \"workspace\" :op \"file-info\" :path \"~A\")"
                       (sexp-escape-lisp path)))
         (reply (ipc-call sexp)))
    (if (and reply (ipc-reply-ok-p reply))
        reply
        "(file not found)")))
