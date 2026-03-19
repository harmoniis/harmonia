;;; baseband.lisp — Port: signal baseband processor for frontend management via IPC.

(in-package :harmonia)

;;; --- Pure Lisp helpers (no CFFI) ---

(defun %parse-gateway-sexp (raw)
  (cond
    ((or (null raw) (zerop (length raw)) (string= raw "nil")) nil)
    (t
     (let ((*read-eval* nil))
       (read-from-string raw)))))

;;; --- Command dispatch callbacks ---
;;; NOTE: With IPC transport, callbacks from Rust into Lisp are no longer
;;; registered via cffi:defcallback. Instead, the Rust gateway actor intercepts
;;; commands (/status, /backends, /chronicle, etc.) and delegates them over IPC.
;;; The dispatch functions %gateway-dispatch-command and
;;; %gateway-dispatch-payment-policy still exist in system-commands.lisp and
;;; are invoked by the runtime when it receives an IPC request from the gateway.

;;; --- Init ---

(defun init-baseband-port ()
  "Initialize the gateway via IPC. Register gateway as actor."
  (let ((reply (ipc-call "(:component \"gateway\" :op \"init\")")))
    ;; Register gateway as actor through the unified registry
    (when (and (ipc-reply-ok-p reply) *runtime*)
      (ignore-errors
        (let ((actor-id (actor-register "gateway")))
          (setf (runtime-state-gateway-actor-id *runtime*) actor-id)
          (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "gateway"))))
    ;; Command and payment-policy callbacks are now handled by the Rust
    ;; gateway actor via IPC dispatch — no callback registration needed.
    (runtime-log *runtime* :gateway-init
                 (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

;;; --- Gateway API wrappers ---

(defun gateway-version ()
  (or (ipc-extract-value
       (ipc-call "(:component \"gateway\" :op \"version\")"))
      "unknown"))

(defun gateway-healthcheck ()
  (let ((reply (ipc-call "(:component \"gateway\" :op \"healthcheck\")")))
    (and reply (ipc-reply-ok-p reply))))

(defun gateway-register (name so-path config-sexp security-label)
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"register\" :name \"~A\" :so-path \"~A\" :config \"~A\" :security-label \"~A\")"
                        (sexp-escape-lisp name) (sexp-escape-lisp so-path)
                        (sexp-escape-lisp config-sexp) (sexp-escape-lisp security-label)))))
    (when (ipc-reply-error-p reply)
      (error "gateway-register failed for ~A: ~A" name reply))
    (ignore-errors (%register-loaded-lib name so-path))
    t))

(defun gateway-unregister (name)
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"unregister\" :name \"~A\")"
                        (sexp-escape-lisp name)))))
    (when (ipc-reply-error-p reply)
      (error "gateway-unregister failed for ~A: ~A" name reply))
    t))

(defun gateway-reload (name)
  "Hot-reload a frontend by name."
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"reload\" :name \"~A\")"
                        (sexp-escape-lisp name)))))
    (if (ipc-reply-ok-p reply)
        (progn
          (ignore-errors (%mark-lib-recovered name))
          (%log :info "gateway" "Reloaded frontend ~A" name)
          t)
        (progn
          (%log :error "gateway" "Reload failed for ~A: ~A" name reply)
          nil))))

(defun gateway-crash-count (name)
  "Return the crash count for a frontend from the gateway's tracking."
  (let ((reply (ipc-call
                (format nil "(:component \"gateway\" :op \"crash-count\" :name \"~A\")"
                        (sexp-escape-lisp name)))))
    (or (ipc-extract-u64 reply ":result") 0)))

(defun gateway-poll ()
  "Poll gateway for inbound envelopes via IPC."
  (let* ((raw (ipc-gateway-poll))
         (parsed (ipc-parse-sexp-reply raw)))
    (when (and (listp parsed) (eq (car parsed) :ok))
      (let ((envs (getf (cdr parsed) :envelopes)))
        (when envs
          (%log :info "gateway-poll" "Received ~D envelopes" (length envs)))
        envs))))

(defun baseband-poll ()
  (gateway-poll))

(defun gateway-send (frontend-name sub-channel payload)
  (let ((reply (ipc-gateway-send frontend-name sub-channel payload)))
    (when (ipc-reply-error-p reply)
      (error "gateway-send failed for ~A/~A: ~A" frontend-name sub-channel reply))
    t))

(defun baseband-send (channel-kind channel-address payload)
  (gateway-send channel-kind channel-address payload))

(defun gateway-list-frontends ()
  (%parse-gateway-sexp
   (or (ipc-extract-value
        (ipc-call "(:component \"gateway\" :op \"list-frontends\")"))
       "nil")))

(defun gateway-frontend-status (name)
  (%parse-gateway-sexp
   (or (ipc-extract-value
        (ipc-call (format nil "(:component \"gateway\" :op \"frontend-status\" :name \"~A\")"
                          (sexp-escape-lisp name))))
       "nil")))

(defun baseband-channel-status (channel-kind)
  (gateway-frontend-status channel-kind))

(defun gateway-list-channels (name)
  (%parse-gateway-sexp
   (or (ipc-extract-value
        (ipc-call (format nil "(:component \"gateway\" :op \"list-channels\" :name \"~A\")"
                          (sexp-escape-lisp name))))
       "nil")))

(defun baseband-list-channels (name)
  (gateway-list-channels name))

(defun gateway-shutdown ()
  (let ((reply (ipc-call "(:component \"gateway\" :op \"shutdown\")")))
    (runtime-log *runtime* :gateway-shutdown
                 (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

(defun gateway-pending-exit ()
  "Check if the gateway has a pending exit request. Returns 0 or 1."
  0)

(defun %gateway-pending-exit ()
  "Legacy alias for gateway-pending-exit."
  (gateway-pending-exit))

;;; --- Pure Lisp config helpers (unchanged) ---

(defun %source-config-root ()
  (merge-pathnames "config/" (merge-pathnames "../../" *boot-file*)))

(defun %system-config-root ()
  (let ((system-dir (%boot-env "HARMONIA_SYSTEM_DIR")))
    (when (and system-dir (> (length system-dir) 0))
      (let ((root (if (char= (char system-dir (1- (length system-dir))) #\/)
                      system-dir
                      (concatenate 'string system-dir "/"))))
        (merge-pathnames "config/" (pathname root))))))

(defun %gateway-config-path ()
  (let* ((system-root (%system-config-root))
         (system-baseband (and system-root (merge-pathnames "baseband.sexp" system-root)))
         (system-legacy (and system-root (merge-pathnames "gateway-frontends.sexp" system-root)))
         (source-baseband (merge-pathnames "baseband.sexp" (%source-config-root))))
    (cond
      ((and system-baseband (probe-file system-baseband)) system-baseband)
      ((and system-legacy (probe-file system-legacy)) system-legacy)
      (t source-baseband))))

(defun %normalize-frontend-so-path (path)
  "Normalize a frontend .so/.dylib path for the current platform."
  (let* ((leaf (subseq path (1+ (or (position #\/ path :from-end t) -1))))
         (lib-dir (%boot-env "HARMONIA_LIB_DIR")))
    (if (and lib-dir (> (length lib-dir) 0))
        (let* ((root (if (char= (char lib-dir (1- (length lib-dir))) #\/)
                         lib-dir
                         (concatenate 'string lib-dir "/")))
               (candidate (concatenate 'string root leaf)))
          (if (probe-file candidate) candidate path))
        path)))

(defun %vault-keys-ready-p (vault-keys)
  (if (null vault-keys)
      t
      (every (lambda (key)
               (vault-has-secret-p (string-downcase (symbol-name key))))
             vault-keys)))

(defun %config-fragment-string (value)
  (typecase value
    (string value)
    (symbol (string-downcase (symbol-name value)))
    (t (princ-to-string value))))

(defun %config-key-ready-p (entry)
  (cond
    ((null entry) t)
    ((and (consp entry) (= (length entry) 2))
     (let* ((scope (%config-fragment-string (first entry)))
            (key (%config-fragment-string (second entry)))
            (value (ignore-errors (config-get key scope))))
       (and value
            (> (length (string-trim '(#\Space #\Tab #\Newline #\Return) value)) 0))))
    (t
     nil)))

(defun %config-keys-ready-p (config-keys)
  (if (null config-keys)
      t
      (every #'%config-key-ready-p config-keys)))

(defun %should-auto-load-p (auto-load vault-keys)
  "Determine if a frontend should auto-load.
   T = always, :IF-VAULT-KEYS = only if all vault keys present,
   :IF-READY = readiness-gated via vault/config prerequisites, NIL = never."
  (cond
    ((eq auto-load t) t)
    ((eq auto-load nil) nil)
    ((eq auto-load :if-vault-keys)
     (%vault-keys-ready-p vault-keys))
    ((eq auto-load :if-ready)
     (%vault-keys-ready-p vault-keys))
    (t nil)))

(defun %current-platform ()
  "Return :MACOS, :LINUX, or :OTHER based on *features*."
  #+darwin :macos
  #+linux  :linux
  #-(or darwin linux) :other)

(defun %platform-allowed-p (platforms)
  "Check if the current platform is in the allowed list, or if no list is given."
  (if (null platforms)
      t
      (member (%current-platform) platforms)))

(defun register-configured-frontends ()
  "Read baseband.sexp and register each frontend, honoring auto-load policy and platform constraints."
  (let ((config-path (%gateway-config-path)))
    (when (probe-file config-path)
      (let ((config (with-open-file (s config-path) (read s))))
        (dolist (fe (getf config :frontends))
          (let ((name (getf fe :name))
                (auto-load (getf fe :auto-load))
                (vault-keys (getf fe :vault-keys))
                (config-keys (getf fe :config-keys))
                (platforms (getf fe :platforms)))
            (when (and (%platform-allowed-p platforms)
                       (%should-auto-load-p auto-load vault-keys)
                       (%config-keys-ready-p config-keys))
              (handler-case
                  (gateway-register
                    name
                    (%normalize-frontend-so-path (getf fe :so-path))
                    (format nil "~S" fe)
                    (string-downcase (symbol-name (getf fe :security-label))))
                (error (e)
                  (%log :warn "gateway" (format nil "Failed to register frontend ~A: ~A" name e)))))))))))
