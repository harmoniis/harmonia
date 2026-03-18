;;; introspection.lisp — Runtime self-knowledge for autonomous debugging and self-repair.
;;;
;;; Gives the agent full awareness of its own runtime environment:
;;; platform, paths, logs, loaded libraries, health, errors, and
;;; how to recompile and hot-reload its own components.

(in-package :harmonia)

;;; ─── Platform detection ────────────────────────────────────────────────

(defun %platform ()
  "Return keyword for current platform."
  #+darwin :macos
  #+linux :linux
  #+freebsd :freebsd
  #+windows :windows
  #-(or darwin linux freebsd windows) :unknown)

(defun %platform-name ()
  (case (%platform)
    (:macos "macOS")
    (:linux "Linux")
    (:freebsd "FreeBSD")
    (:windows "Windows")
    (t "unknown")))

;;; ─── Environment access ─────────────────────────────────────────────
;;;
;;; %boot-env is the ONLY function that reads env vars directly.
;;; Pre-config-store code uses %boot-env; post-config-store code uses
;;; config-get-for (which handles env fallback internally).

(defun %boot-env (name &optional default)
  "Read an environment variable. Use ONLY for pre-config-store bootstrap paths.
   After config-store init, use config-get-for instead."
  (or (sb-ext:posix-getenv name) default))

(defun %platform-tmpdir ()
  "Platform temporary directory."
  (%boot-env "TMPDIR" "/tmp"))

(defun %tmpdir-state-root ()
  "State root from config-store, falling back to $TMPDIR/harmonia.
   Use in post-config-store code where config-get-for is available."
  (or (and (fboundp 'config-get-for)
           (funcall 'config-get-for "conductor" "state-root" "global"))
      (concatenate 'string (string-right-trim "/" (%platform-tmpdir)) "/harmonia")))

;;; ─── Path introspection ────────────────────────────────────────────────

(defun %state-root ()
  (or (%boot-env "HARMONIA_STATE_ROOT")
      (let ((home (%boot-env "HOME")))
        (when home (concatenate 'string home "/.harmoniis/harmonia")))))

(defun %source-root ()
  "Where boot.lisp lives — the Lisp source tree root."
  (when *boot-file*
    (namestring (merge-pathnames "../../" *boot-file*))))

(defun %lib-dir ()
  (or (%boot-env "HARMONIA_LIB_DIR")
      (let ((home (%boot-env "HOME")))
        (when home (concatenate 'string home "/.local/lib/harmonia/")))))

(defun %log-path ()
  "Platform-correct log path."
  (case (%platform)
    (:macos
     (let ((home (%boot-env "HOME")))
       (when home (concatenate 'string home "/Library/Logs/Harmonia/harmonia.log"))))
    (:linux
     (let ((state (or (%boot-env "XDG_STATE_HOME")
                      (let ((h (%boot-env "HOME")))
                        (when h (concatenate 'string h "/.local/state"))))))
       (when state (concatenate 'string state "/harmonia/harmonia.log"))))
    (t (let ((root (%state-root)))
         (when root (concatenate 'string root "/harmonia.log"))))))

(defun %runtime-dir ()
  "Platform-correct runtime directory (PID/socket)."
  (case (%platform)
    (:macos
     (concatenate 'string (string-right-trim "/" (%platform-tmpdir)) "/harmonia/"))
    (:linux
     (let ((xdg (%boot-env "XDG_RUNTIME_DIR")))
       (if xdg
           (concatenate 'string xdg "/harmonia/")
           "/tmp/harmonia/")))
    (t "/tmp/harmonia/")))

;;; ─── Library introspection ─────────────────────────────────────────────

(defparameter *loaded-libs*
  (make-hash-table :test 'equal)
  "Map of library-name → (:path ... :loaded-at ... :crash-count ... :status ...)")

(defun %register-loaded-lib (name path)
  "Record that a library was loaded."
  (setf (gethash name *loaded-libs*)
        (list :path path
              :loaded-at (get-universal-time)
              :crash-count 0
              :last-crash nil
              :status :running)))

(defun %record-lib-crash (name error-detail)
  "Record a library crash."
  (let ((entry (gethash name *loaded-libs*)))
    (when entry
      (setf (getf entry :crash-count) (1+ (or (getf entry :crash-count) 0)))
      (setf (getf entry :last-crash) (list :time (get-universal-time)
                                            :detail error-detail))
      (setf (getf entry :status) :crashed)
      (setf (gethash name *loaded-libs*) entry)))
  (%log :error "supervisor" "Library ~A crashed: ~A" name error-detail)
  (ignore-errors
    (harmonic-matrix-log-event "supervisor" "lib-crash" name error-detail nil "")))

(defun %mark-lib-recovered (name)
  "Mark a library as recovered after reload."
  (let ((entry (gethash name *loaded-libs*)))
    (when entry
      (setf (getf entry :status) :running)
      (setf (getf entry :loaded-at) (get-universal-time))
      (setf (gethash name *loaded-libs*) entry)))
  (%log :info "supervisor" "Library ~A recovered." name))

(defun introspect-libs ()
  "Return status of all loaded libraries."
  (let ((result '()))
    (maphash (lambda (name entry)
               (push (list* :name name entry) result))
             *loaded-libs*)
    result))

;;; ─── Error history ─────────────────────────────────────────────────────

(defparameter *error-ring* (make-array 64 :initial-element nil)
  "Circular buffer of recent errors for self-diagnosis.")
(defparameter *error-ring-index* 0)

(defun %push-error-ring (error-record)
  "Push an error into the ring buffer."
  (setf (aref *error-ring* (mod *error-ring-index* (length *error-ring*)))
        error-record)
  (incf *error-ring-index*))

(defun introspect-recent-errors (&optional (limit 10))
  "Return the N most recent errors."
  (let ((results '())
        (ring-len (length *error-ring*))
        (count (min limit (min *error-ring-index* (length *error-ring*)))))
    (dotimes (i count)
      (let* ((idx (mod (- *error-ring-index* 1 i) ring-len))
             (entry (aref *error-ring* idx)))
        (when entry (push entry results))))
    (nreverse results)))

;;; ─── Self-compilation ──────────────────────────────────────────────────

(defun %cargo-build-component (crate-name)
  "Build a single workspace crate in release mode. Returns (values success-p output)."
  (let* ((source-root (%source-root))
         (cmd (format nil "cd ~A && cargo build --release -p ~A 2>&1"
                      source-root crate-name)))
    (%log :info "self-compile" "Building ~A..." crate-name)
    (multiple-value-bind (output err-output exit-code)
        (ignore-errors
          (let ((proc (sb-ext:run-program "/bin/sh" (list "-c" cmd)
                                          :output :stream
                                          :error :output
                                          :wait t)))
            (let ((out (with-output-to-string (s)
                         (let ((stream (sb-ext:process-output proc)))
                           (loop for line = (read-line stream nil nil)
                                 while line
                                 do (write-line line s))))))
              (values out nil (sb-ext:process-exit-code proc)))))
      (declare (ignore err-output))
      (let ((success (and exit-code (zerop exit-code))))
        (if success
            (%log :info "self-compile" "Built ~A successfully." crate-name)
            (%log :error "self-compile" "Build failed for ~A (exit=~A)" crate-name exit-code))
        (values success (or output ""))))))

(defun %dylib-name-for-crate (crate-name)
  "Convert a Cargo crate name to the output dylib filename."
  (let ((base (substitute #\_ #\- crate-name)))
    (format nil "lib~A.~A" base (%shared-lib-extension))))

(defun %hot-reload-frontend (frontend-name crate-name &optional config-sexp)
  "Rebuild a frontend crate, copy the new dylib, and hot-reload via gateway.
   Returns (values success-p detail)."
  (multiple-value-bind (build-ok build-output)
      (%cargo-build-component crate-name)
    (unless build-ok
      (return-from %hot-reload-frontend
        (values nil (format nil "build failed: ~A" build-output))))
    ;; Copy new dylib to lib dir
    (let* ((dylib-name (%dylib-name-for-crate crate-name))
           (source-path (merge-pathnames dylib-name
                                         (pathname (concatenate 'string (%source-root) "target/release/"))))
           (dest-dir (%lib-dir))
           (dest-path (when dest-dir (merge-pathnames dylib-name (pathname dest-dir)))))
      (when (and dest-path (probe-file source-path))
        (handler-case
            (progn
              (with-open-file (in source-path :element-type '(unsigned-byte 8))
                (with-open-file (out dest-path :element-type '(unsigned-byte 8)
                                               :direction :output
                                               :if-exists :supersede)
                  (let ((buf (make-array 65536 :element-type '(unsigned-byte 8))))
                    (loop for n = (read-sequence buf in)
                          while (plusp n)
                          do (write-sequence buf out :end n)))))
              (%log :info "self-compile" "Copied ~A to ~A" dylib-name dest-dir))
          (error (e)
            (return-from %hot-reload-frontend
              (values nil (format nil "copy failed: ~A" e))))))
      ;; Reload via gateway
      (handler-case
          (progn
            (ignore-errors (gateway-unregister frontend-name))
            (let* ((so-path (if dest-path
                                (namestring dest-path)
                                (namestring source-path)))
                   (cfg (or config-sexp "()")))
              (gateway-register frontend-name so-path cfg "owner")
              (%mark-lib-recovered frontend-name)
              (values t (format nil "reloaded ~A from ~A" frontend-name so-path))))
        (error (e)
          (values nil (format nil "reload failed: ~A" e)))))))

;;; ─── Full runtime snapshot for self-diagnosis ──────────────────────────

(defun introspect-runtime ()
  "Return a complete self-diagnostic snapshot the agent can reason about."
  (list
   :platform (%platform-name)
   :environment (and *runtime* (runtime-state-environment *runtime*))
   :cycle (and *runtime* (runtime-state-cycle *runtime*))
   :uptime-seconds (and *runtime*
                        (- (get-universal-time) (runtime-state-started-at *runtime*)))
   :queue-depth (and *runtime* (length (runtime-state-prompt-queue *runtime*)))
   :tool-count (and *runtime* (hash-table-count (runtime-state-tools *runtime*)))
   :rewrite-count (and *runtime* (runtime-state-rewrite-count *runtime*))
   :harmonic-phase (and *runtime* (runtime-state-harmonic-phase *runtime*))
   :paths (list :state-root (%state-root)
                :source-root (%source-root)
                :lib-dir (%lib-dir)
                :log-path (%log-path)
                :runtime-dir (%runtime-dir))
   :libraries (introspect-libs)
   :recent-errors (introspect-recent-errors 5)
   :frontends (ignore-errors (gateway-list-frontends))))

;;; ─── Self-knowledge for DNA system prompt ──────────────────────────────

(defun %runtime-self-knowledge ()
  "Generate a block of self-knowledge text for the DNA system prompt.
   Tells the agent what it is, where things are, and how to fix itself."
  (format nil
"RUNTIME SELF-KNOWLEDGE
Platform: ~A
Source: ~A
Libraries: ~A
Logs: ~A
State: ~A

SELF-REPAIR CAPABILITIES
- Read logs: (introspect-runtime) returns full diagnostic snapshot
- View errors: (introspect-recent-errors N) returns last N errors with context
- Library status: (introspect-libs) shows all loaded cdylibs and crash counts
- Rebuild component: (%cargo-build-component \"crate-name\") compiles a single crate
- Hot-reload frontend: (%hot-reload-frontend \"name\" \"crate-name\") rebuilds + reloads a frontend cdylib
- Record crash: (%record-lib-crash \"name\" \"detail\") logs a library failure
- Gateway reload: (gateway-unregister \"name\") then (gateway-register ...) to hot-swap

ARCHITECTURE
- Core loop: src/core/loop.lisp — tick-based, never crashes, all actions wrapped in handler-case
- Each tick: gateway-poll -> process-prompt -> memory-heartbeat -> harmonic-step -> gateway-flush
- FFI calls through CFFI to Rust cdylibs; crashes are caught and recorded, not propagated
- Frontends are cdylibs loaded via gateway-register; can be unregistered and reloaded at runtime
- Source rewrite via ouroboros: write patch, validate with rust-forge, snapshot version, reload

DEBUGGING GUIDE
- Check logs: tail -f ~A
- Check library health: (introspect-libs)
- Check recent errors: (introspect-recent-errors 10)
- If a frontend stops responding: (%hot-reload-frontend \"name\" \"crate-name\")
- If orchestration fails: check (introspect-recent-errors) for backend errors
- If memory/config issues: check state-root ~A for vault.db, config.db
- To recompile everything: (%cargo-build-component \"harmonia\") from source root"
          (%platform-name)
          (or (%source-root) "unknown")
          (or (%lib-dir) "unknown")
          (or (%log-path) "unknown")
          (or (%state-root) "unknown")
          (or (%log-path) "unknown")
          (or (%state-root) "unknown")))
