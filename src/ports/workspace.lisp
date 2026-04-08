;;; workspace.lisp — Port: Workspace file operations via IPC actor.
;;;
;;; The agent's hands for reading the world. All file operations go through
;;; the Rust workspace actor (parallel, async, path-sandboxed).
;;; The LLM controls these through REPL only — tools are actors, not Lisp code.
;;;
;;; HOMOICONIC: all IPC commands are built as Lisp LISTS, then serialized
;;; with %sexp-to-ipc-string. S-expressions are law — no format strings.

(in-package :harmonia)

(defun init-workspace-port ()
  "Initialize workspace port. Verifies the actor responds."
  (let ((reply (handler-case
     (ipc-call (%sexp-to-ipc-string
                             '(:component "workspace" :op "healthcheck")
   (error () nil))))))
    (if (and reply (ipc-reply-ok-p reply))
        (progn (%log :info "workspace" "Workspace actor ready")
               t)
        (progn (%log :warn "workspace" "Workspace actor not available")
               nil))))

;;; --- Public API ---

(defun workspace-read-file (path &key (offset 0) (limit 200))
  "Read file lines via workspace actor. Returns result string."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "read-file"
                    :path ,path :offset ,offset :limit ,limit))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun workspace-grep (pattern &optional (path ".") &key (limit 30))
  "Grep workspace for pattern via actor. Returns result string."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "grep"
                    :pattern ,pattern :path ,path :limit ,limit))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun workspace-list-files (&optional (path ".") &key (pattern "") (limit 50))
  "List files in directory via workspace actor."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "list-files"
                    :path ,path :pattern ,pattern :limit ,limit)))))
    (if (and reply (ipc-reply-ok-p reply))
        reply
        "(no results)")))

(defun workspace-file-exists-p (path)
  "Check if file exists via workspace actor."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "file-exists" :path ,path))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (eq t (getf (cdr parsed) :exists)))))

(defun workspace-file-info (path)
  "Get file size/lines/type via workspace actor."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "file-info" :path ,path)))))
    (if (and reply (ipc-reply-ok-p reply))
        reply
        "(file not found)")))

(defun workspace-exec (cmd args)
  "Execute shell command via workspace actor. Full terminal power."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "exec"
                    :cmd ,cmd :args ,(format nil "~{~A~^ ~}" args)))))
         (*read-eval* nil)
         (parsed (when (and reply (ipc-reply-ok-p reply))
                   (handler-case (read-from-string reply) (error () nil)))))
    (when (listp parsed) (getf (cdr parsed) :result))))

(defun workspace-write-file (path content)
  "Write/create file via workspace actor."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "write-file"
                    :path ,path :content ,content)))))
    (if (and reply (ipc-reply-ok-p reply))
        (format nil "Written: ~A" path)
        "(write-file: error)")))

(defun workspace-append-file (path content)
  "Append to file via workspace actor."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "workspace" :op "append-file"
                    :path ,path :content ,content)))))
    (if (and reply (ipc-reply-ok-p reply))
        (format nil "Appended: ~A" path)
        "(append-file: error)")))
