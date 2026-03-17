;;; baseband.lisp — Port: signal baseband processor for frontend management via gateway CFFI.

(in-package :harmonia)

(defparameter *gateway-lib* nil)

(cffi:defcfun ("harmonia_gateway_version" %gateway-version) :string)
(cffi:defcfun ("harmonia_gateway_healthcheck" %gateway-healthcheck) :int)
(cffi:defcfun ("harmonia_gateway_init" %gateway-init) :int)
(cffi:defcfun ("harmonia_gateway_register" %gateway-register) :int
  (name :string)
  (so-path :string)
  (config-sexp :string)
  (security-label :string))
(cffi:defcfun ("harmonia_gateway_unregister" %gateway-unregister) :int
  (name :string))
(cffi:defcfun ("harmonia_gateway_poll" %gateway-poll) :string)
(cffi:defcfun ("harmonia_gateway_send" %gateway-send) :int
  (frontend-name :string)
  (sub-channel :string)
  (payload :string))
(cffi:defcfun ("harmonia_gateway_list_frontends" %gateway-list-frontends) :string)
(cffi:defcfun ("harmonia_gateway_frontend_status" %gateway-frontend-status) :string
  (name :string))
(cffi:defcfun ("harmonia_gateway_list_channels" %gateway-list-channels) :string
  (name :string))
(cffi:defcfun ("harmonia_gateway_shutdown" %gateway-shutdown) :int)
(cffi:defcfun ("harmonia_gateway_reload" %gateway-reload) :int
  (name :string))
(cffi:defcfun ("harmonia_gateway_crash_count" %gateway-crash-count) :int
  (name :string))
(cffi:defcfun ("harmonia_gateway_free_string" %gateway-free-string) :void
  (ptr :pointer))
(cffi:defcfun ("harmonia_gateway_set_command_query" %gateway-set-command-query) :int
  (handler :pointer))
(cffi:defcfun ("harmonia_gateway_set_payment_policy_query" %gateway-set-payment-policy-query) :int
  (handler :pointer))
(cffi:defcfun ("harmonia_gateway_pending_exit" %gateway-pending-exit) :int)

(defun %parse-gateway-sexp (raw)
  (cond
    ((or (null raw) (zerop (length raw)) (string= raw "nil")) nil)
    (t
     (let ((*read-eval* nil))
       (read-from-string raw)))))

;;; ─── Command dispatch callback ─────────────────────────────────────────
;;; The gateway calls this for delegated commands (/status, /backends, etc.)
;;; that need Lisp runtime state. The gateway has already enforced security.

(cffi:defcallback gateway-command-dispatch :pointer
    ((command :string) (args :string))
  (handler-case
      (let ((result (%gateway-dispatch-command command args)))
        (if result
            (cffi:foreign-string-alloc result)
            (cffi:null-pointer)))
    (error (e)
      (cffi:foreign-string-alloc
       (format nil "[system] Error executing ~A: ~A" command e)))))

(cffi:defcallback gateway-payment-policy-dispatch :pointer
    ((summary :string))
  (handler-case
      (let ((result (%gateway-dispatch-payment-policy summary)))
        (if result
            (cffi:foreign-string-alloc result)
            (cffi:null-pointer)))
    (error (e)
      (runtime-log *runtime* :gateway-payment-policy-error (list :error (format nil "~A" e)))
      (cffi:null-pointer))))

(defun init-baseband-port ()
  (ensure-cffi)
  (setf *gateway-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_gateway.dylib")))
  (let ((rc (%gateway-init)))
    ;; Register gateway as actor through the unified registry (in parallel-agents dylib)
    (when (and (zerop rc) *runtime*)
      (ignore-errors
        (let ((actor-id (actor-register "gateway")))
          (setf (runtime-state-gateway-actor-id *runtime*) actor-id)
          (setf (gethash actor-id (runtime-state-actor-kinds *runtime*)) "gateway"))))
    ;; Register the command dispatch callback so the gateway can delegate
    ;; /status, /backends, /chronicle etc. back to Lisp for runtime state access
    (when (zerop rc)
      (let ((cb-rc (%gateway-set-command-query
                    (cffi:callback gateway-command-dispatch))))
        (when (zerop cb-rc)
          (runtime-log *runtime* :gateway-command-query (list :status "registered")))))
    (when (zerop rc)
      (let ((cb-rc (%gateway-set-payment-policy-query
                    (cffi:callback gateway-payment-policy-dispatch))))
        (when (zerop cb-rc)
          (runtime-log *runtime* :gateway-payment-policy-query (list :status "registered")))))
    (runtime-log *runtime* :gateway-init (list :status rc))
    (zerop rc)))

(defun gateway-version ()
  (%gateway-version))

(defun gateway-healthcheck ()
  (= (%gateway-healthcheck) 1))

(defun gateway-register (name so-path config-sexp security-label)
  (let ((rc (%gateway-register name so-path config-sexp security-label)))
    (unless (zerop rc)
      (error "gateway-register failed for ~A (rc=~D)" name rc))
    (ignore-errors (%register-loaded-lib name so-path))
    t))

(defun gateway-unregister (name)
  (let ((rc (%gateway-unregister name)))
    (unless (zerop rc)
      (error "gateway-unregister failed for ~A (rc=~D)" name rc))
    t))

(defun gateway-reload (name)
  "Hot-reload a frontend by name. The gateway unloads, re-dlopen, and re-init."
  (let ((rc (%gateway-reload name)))
    (if (zerop rc)
        (progn
          (ignore-errors (%mark-lib-recovered name))
          (%log :info "gateway" "Reloaded frontend ~A" name)
          t)
        (progn
          (%log :error "gateway" "Reload failed for ~A (rc=~D)" name rc)
          nil))))

(defun gateway-crash-count (name)
  "Return the crash count for a frontend from the gateway's tracking."
  (%gateway-crash-count name))

(defun gateway-poll ()
  (%parse-gateway-sexp (%gateway-poll)))

(defun baseband-poll ()
  (gateway-poll))

(defun gateway-send (frontend-name sub-channel payload)
  (let ((rc (%gateway-send frontend-name sub-channel payload)))
    (unless (zerop rc)
      (error "gateway-send failed for ~A/~A (rc=~D)" frontend-name sub-channel rc))
    t))

(defun baseband-send (channel-kind channel-address payload)
  (gateway-send channel-kind channel-address payload))

(defun gateway-list-frontends ()
  (%parse-gateway-sexp (%gateway-list-frontends)))

(defun gateway-frontend-status (name)
  (%parse-gateway-sexp (%gateway-frontend-status name)))

(defun baseband-channel-status (channel-kind)
  (gateway-frontend-status channel-kind))

(defun gateway-list-channels (name)
  (%parse-gateway-sexp (%gateway-list-channels name)))

(defun baseband-list-channels (name)
  (gateway-list-channels name))

(defun gateway-shutdown ()
  (let ((rc (%gateway-shutdown)))
    (runtime-log *runtime* :gateway-shutdown (list :status rc))
    (zerop rc)))

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
  (let* ((base (if (search "target/release/" path)
                   (concatenate 'string
                                "target/release/"
                                (subseq path (1+ (position #\/ path :from-end t))))
                   path))
         (dot (position #\. base :from-end t))
         (stem (if dot (subseq base 0 dot) base))
         (ext (if dot (string-downcase (subseq base (1+ dot))) ""))
         (normalized (if (member ext '("dylib" "so" "dll") :test #'string=)
                         (concatenate 'string stem "." (%shared-lib-extension))
                         (concatenate 'string base "." (%shared-lib-extension))))
         (lib-dir (%boot-env "HARMONIA_LIB_DIR")))
    (if (and lib-dir (> (length lib-dir) 0))
        (let* ((root (if (char= (char lib-dir (1- (length lib-dir))) #\/)
                         lib-dir
                         (concatenate 'string lib-dir "/")))
               (leaf (subseq normalized (1+ (position #\/ normalized :from-end t))))
               (candidate (concatenate 'string root leaf)))
          (if (probe-file candidate) candidate normalized))
        normalized)))

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
