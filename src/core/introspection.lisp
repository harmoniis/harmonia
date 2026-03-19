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

;;; %hot-reload-frontend and %dylib-name-for-crate removed — cdylib/FFI architecture
;;; replaced by harmonia-runtime with ractor actors and IPC (Unix domain socket).

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
   :phoenix-health (%phoenix-health)
   :libraries (introspect-libs)
   :recent-errors (introspect-recent-errors 5)
   :frontends (ignore-errors (gateway-list-frontends))))

;;; ─── Self-knowledge for DNA system prompt ──────────────────────────────

(defun %phoenix-health-url ()
  "URL for the Phoenix supervisor health endpoint (localhost only)."
  "http://127.0.0.1:9100/health")

(defun %phoenix-health ()
  "Query Phoenix health endpoint. Returns the JSON string or nil on failure."
  (ignore-errors
    (let ((result (hfetch (%phoenix-health-url) :timeout-ms 3000)))
      (when (and result (stringp result))
        result))))

(defun %phoenix-health-formatted ()
  "Query Phoenix health and return formatted string for TUI display."
  (let ((raw (%phoenix-health)))
    (if raw
        (format nil "  ~A" raw)
        "  Phoenix health endpoint unreachable (127.0.0.1:9100)")))

(defun %phoenix-identity-block ()
  "Compact Phoenix identity block injected into ALL DNA prompt modes.
   Prevents hallucination about Elixir Phoenix."
  "
PHOENIX PROCESS SUPERVISOR (CRITICAL — READ THIS)
'Phoenix' in Harmonia means YOUR OWN process supervisor (harmonia-phoenix),
NOT the Elixir/Erlang web framework. Never confuse them.
Phoenix is the root daemon that supervises all Harmonia processes:
  Phoenix (ractor supervisor, health HTTP at 127.0.0.1:9100)
    +-- harmonia-runtime (Rust binary, ractor actors, IPC via Unix socket)
    +-- sbcl-agent (you — this SBCL process)
    +-- provision-server
To check your own health status: use hfetch on http://127.0.0.1:9100/health
The response is JSON with: mode (Full/Starting/Degraded/CoreOnly), uptime, subsystem states.
The /status command shows Phoenix health. The /diagnose command runs deep self-diagnosis.
When asked about your health, crashes, or Phoenix — ALWAYS query the real endpoint first.
Never fabricate health data. If the endpoint is unreachable, say so honestly.")

(defun %runtime-self-knowledge ()
  "Generate a block of self-knowledge text for the DNA system prompt.
   Tells the agent what it is, where things are, and how to fix itself."
  (format nil
"RUNTIME SELF-KNOWLEDGE
CRITICAL: 'Phoenix' means YOUR process supervisor (harmonia-phoenix at 127.0.0.1:9100), NOT the Elixir web framework.
Platform: ~A
Source: ~A
State: ~A
Logs: ~A

PROCESS ARCHITECTURE
You (SBCL) are supervised by Phoenix, which manages all Harmonia processes.
Phoenix is the root process (PID 1 of the agent) that reads phoenix.toml
and supervises: harmonia-runtime, sbcl-agent (you), and provision-server.

  Phoenix (ractor supervisor, writes phoenix.pid, health HTTP at 127.0.0.1:9100)
    ├─ harmonia-runtime (Rust binary, all ractor actors, IPC via Unix socket)
    ├─ sbcl-agent (you — this SBCL process)
    └─ provision-server

You communicate with Rust actors via IPC (Unix domain socket at $STATE_ROOT/runtime.sock).
All Rust crates are compiled as rlib into harmonia-runtime — no more cdylib/FFI/dlopen.

PHOENIX HEALTH ENDPOINT
  GET http://127.0.0.1:9100/health     → JSON: mode, uptime, all subsystem states
  GET http://127.0.0.1:9100/health/ready → 200 if Full, 503 otherwise

  Daemon modes:
    Full     — all subsystems running, system healthy
    Starting — subsystems still coming up
    Degraded — non-core subsystem failed, core OK
    CoreOnly — a core subsystem failed

  To check your own status: (hfetch \"http://127.0.0.1:9100/health\")
  The health endpoint is localhost-only (127.0.0.1), PIDs are redacted.

SELF-REPAIR CAPABILITIES
- Check Phoenix health: (hfetch \"http://127.0.0.1:9100/health\") — your daemon status
- Read logs: (introspect-runtime) returns full diagnostic snapshot
- View errors: (introspect-recent-errors N) returns last N errors with context
- Library status: (introspect-libs) shows loaded modules and crash counts
- Rebuild component: (%cargo-build-component \"crate-name\") compiles a single crate
- Record crash: (%record-lib-crash \"name\" \"detail\") logs a module failure
- Source rewrite via ouroboros: write patch, validate with rust-forge, snapshot version

CORE LOOP
- src/core/loop.lisp — tick-based, never crashes, all actions wrapped in handler-case
- Each tick: gateway-poll -> process-prompt -> memory-heartbeat -> harmonic-step -> gateway-flush

DEBUGGING GUIDE
- Check Phoenix status: (hfetch \"http://127.0.0.1:9100/health\") — JSON with mode + subsystems
- Check logs: tail -f ~A
- Check library health: (introspect-libs)
- Check recent errors: (introspect-recent-errors 10)
- If orchestration fails: check (introspect-recent-errors) for backend errors
- If memory/config issues: check state-root ~A for vault.db, config.db
- To recompile: (%cargo-build-component \"harmonia-runtime\") from source root
- If Phoenix shows Degraded/CoreOnly: operator should run 'harmonia status' from CLI"
          (%platform-name)
          (or (%source-root) "unknown")
          (or (%state-root) "unknown")
          (or (%log-path) "unknown")
          (or (%log-path) "unknown")
          (or (%state-root) "unknown")))
