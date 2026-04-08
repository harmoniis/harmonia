;;; swarm-tmux.lisp — Tmux Lisp wrappers (via IPC component dispatch).

(in-package :harmonia)

;;; --- Tmux Lisp wrappers (via IPC component dispatch) ---

(defun tmux-spawn (cli-type workdir prompt)
  "Spawn a tmux CLI agent. Returns agent id (>= 0) or signals error."
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "tmux" :op "spawn"
                    :cli-type ,cli-type :workdir ,workdir
                    :prompt ,(or prompt "")))))
         (id (when reply (ipc-extract-u64 reply ":task-id"))))
    (unless (and id (>= id 0))
      (error "tmux spawn failed: ~A" (or reply "no reply")))
    id))

(defun tmux-poll (id)
  "Poll a tmux agent state. Returns sexp string."
  (ipc-extract-value
   (ipc-call (%sexp-to-ipc-string
              `(:component "tmux" :op "poll" :id ,id)))))

(defun tmux-kill (id)
  "Kill a tmux agent."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "kill" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux kill failed: ~A" reply))
    t))

(defun tmux-capture (id &optional (history 200))
  "Capture terminal output of a tmux agent."
  (ipc-extract-value
   (ipc-call (%sexp-to-ipc-string
              `(:component "tmux" :op "capture" :id ,id :history ,history)))))

(defun tmux-swarm-poll ()
  "Poll all active tmux agents."
  (ipc-extract-value
   (ipc-call (%sexp-to-ipc-string
              '(:component "tmux" :op "swarm-poll")))))

(defun tmux-send-input (id input)
  "Send text input followed by Enter to a tmux CLI agent."
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "tmux" :op "send" :id ,id
                   :input ,(or input ""))))))
    (when (ipc-reply-error-p reply)
      (error "tmux send failed: ~A" reply))
    t))

(defun tmux-send-key (id key)
  "Send a special key (Enter, Tab, Escape, Up, Down, C-c, etc.) to a tmux agent."
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "tmux" :op "send-key" :id ,id
                   :key ,(or key ""))))))
    (when (ipc-reply-error-p reply)
      (error "tmux send-key failed: ~A" reply))
    t))

(defun tmux-approve (id)
  "Approve a permission prompt on a tmux CLI agent."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "approve" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux approve failed: ~A" reply))
    t))

(defun tmux-deny (id)
  "Deny a permission prompt on a tmux CLI agent."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "deny" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux deny failed: ~A" reply))
    t))

(defun tmux-confirm-yes (id)
  "Confirm yes on a tmux CLI agent confirmation prompt."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "confirm-yes" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux confirm-yes failed: ~A" reply))
    t))

(defun tmux-confirm-no (id)
  "Confirm no on a tmux CLI agent confirmation prompt."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "confirm-no" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux confirm-no failed: ~A" reply))
    t))

(defun tmux-select-option (id index)
  "Select option by INDEX (0-based) on a tmux CLI agent selection menu."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "select" :id ,id :index ,index)))))
    (when (ipc-reply-error-p reply)
      (error "tmux select failed: ~A" reply))
    t))

(defun tmux-interrupt (id)
  "Send Ctrl+C interrupt to a tmux CLI agent."
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           `(:component "tmux" :op "interrupt" :id ,id)))))
    (when (ipc-reply-error-p reply)
      (error "tmux interrupt failed: ~A" reply))
    t))
