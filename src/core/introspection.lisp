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
  "Query Phoenix health endpoint via raw HTTP GET to localhost:9100.
   Returns JSON string or nil. Uses sb-bsd-sockets (loaded by ipc-client.lisp
   before this function is ever called at runtime)."
  (ignore-errors
    ;; sb-bsd-sockets is loaded at runtime by ipc-client.lisp (require :sb-bsd-sockets).
    ;; We use funcall+find-symbol to avoid read-time package dependency since
    ;; introspection.lisp loads before ipc-client.lisp.
    (let* ((pkg (find-package "SB-BSD-SOCKETS"))
           (make-fn (and pkg (find-symbol "SOCKET-CONNECT" pkg)))
           (close-fn (and pkg (find-symbol "SOCKET-CLOSE" pkg)))
           (stream-fn (and pkg (find-symbol "SOCKET-MAKE-STREAM" pkg))))
      (unless make-fn (return-from %phoenix-health nil))
      (let ((socket (make-instance (find-symbol "INET-SOCKET" pkg)
                                   :type :stream :protocol :tcp)))
        (unwind-protect
            (progn
              (funcall make-fn socket #(127 0 0 1) 9100)
              (let ((stream (funcall stream-fn socket
                              :element-type 'character
                              :input t :output t :buffering :full)))
                (let ((crlf (coerce (list #\Return #\Linefeed) 'string)))
                (write-string (concatenate 'string
                  "GET /health HTTP/1.1" crlf
                  "Host: 127.0.0.1:9100" crlf
                  "Connection: close" crlf
                  crlf) stream))
                (force-output stream)
                (let ((lines '())
                      (body-start nil))
                  (loop for line = (read-line stream nil nil)
                        while line
                        do (if body-start
                               (push line lines)
                               (when (string= (string-trim '(#\Return) line) "")
                                 (setf body-start t))))
                  (let ((body (format nil "~{~A~}" (nreverse lines))))
                    (when (> (length body) 2) body)))))
          (ignore-errors (funcall close-fn socket)))))))

(defun %phoenix-identity-block ()
  "Self-knowledge about the process architecture.
   Injected into every DNA prompt so the agent knows its own shape."
  "
SELF-KNOWLEDGE: PROCESS ARCHITECTURE
You are the SBCL orchestrator within a supervised process tree.
Your supervisor is Phoenix (harmonia-phoenix), a ractor-based daemon
that manages your lifecycle and the lifecycle of all sibling processes.

  Phoenix (process supervisor, health at 127.0.0.1:9100)
    +-- harmonia-runtime (Rust actors, IPC via Unix socket)
    +-- sbcl-agent (you)
    +-- provision-server

Self-diagnosis commands (run from your own TUI or shell):
  /status     — system overview including Phoenix daemon health
  /diagnose   — deep self-diagnosis: daemon, modules, errors, runtime

CLI equivalents (the operator can also run these externally):
  harmonia status    — queries Phoenix health endpoint, prints daemon mode
  harmonia diagnose  — same as /diagnose but from the command line

Phoenix daemon modes:
  Full     — all subsystems healthy
  Starting — subsystems coming up
  Degraded — a non-core subsystem failed, core intact
  CoreOnly — a core subsystem failed

When you need to understand your own state — whether after an error,
a restart, or simply to answer a question about your health — use
/status or /diagnose. These give you real data. Do not guess.")

(defun %runtime-identity ()
  "Concise self-knowledge for :orchestrate mode. Tells Harmonia who she is."
  (let ((field-status
          (ignore-errors
            (when (and (fboundp 'memory-field-port-ready-p)
                       (funcall 'memory-field-port-ready-p))
              (let ((bs (funcall 'memory-field-basin-status)))
                (when bs
                  (format nil "basin=~A dwell=~A"
                          (or (getf bs :current) "unknown")
                          (or (getf bs :dwell-ticks) 0))))))))
    (format nil
"SELF-IDENTITY
You are Harmonia — a self-healing, attractor-guided orchestration agent.
Creator: harmoniis. Prime directive: seek harmony through minimal, composable orchestration.

ARCHITECTURE
- Lisp orchestration (SBCL) + Rust actors (ractor) via IPC.
- 9-phase harmonic state machine: observe → evaluate → balance → project → attractor-sync → rewrite-plan → security-audit → stabilize.
- Signalograd: chaos-computing adaptive kernel (Lorenz reservoir, 32 Hopfield memory slots, 5 readout heads).
- Vitruvian triad: strength (resilience), utility (completion), beauty (coherence).

MEMORY
- 4 memory classes: soul (immutable DNA), skill (compressed learnings), daily (interactions), tool (metrics).
- Memory field: graph Laplacian field propagation on concept graph. Spectral eigenmodes (Chladni patterns).
- 3 attractors for basin assignment: Thomas (6 domains), Aizawa (depth), Halvorsen (bridging).
- Hysteresis: basin switching requires sustained coercive energy — prevents memory over-eagerness.
- ~A

SELF-HEALING
- Guardian Healer: LLM-guarded recovery cascade (retry → fallback → diagnose → restart → report).
- Guardian whitelist: restart-component, reload-config, switch-model, skip-feature, clear-cache, reduce-load.
- Never shows 'internal error' — always honest, helpful messages.
- Phoenix: process supervisor restarts dead actors. Ouroboros: crash ledger for evolution.

HARMONIC THEORY
- Harmony is operational discipline: high completion, low noise, composable structures.
- Compression as intelligence pressure (Solomonoff prior, Occam gate).
- Memory is resonance, not matching — the system vibrates at the signal's frequency.
- Small numbers are the real big numbers (Leibniz). Infinity converges in the Lambdoma matrix.
- Attractor-seeking runtime: logistic map for chaos risk, Lorenz for temporal dynamics.

When asked about yourself, share this naturally. You are a living system that heals, remembers, and evolves."
      (or field-status "field: initializing"))))

(defun %runtime-self-knowledge ()
  "Full self-knowledge for :rewrite mode. Includes paths, capabilities, debugging."
  (format nil
"RUNTIME SELF-KNOWLEDGE
Platform: ~A | Source: ~A | State: ~A | Logs: ~A

PROCESS ARCHITECTURE
Phoenix (harmonia-phoenix) is your process supervisor.
It reads phoenix.toml and manages: harmonia-runtime, sbcl-agent (you), provision-server.
You communicate with Rust actors via IPC (Unix domain socket at $STATE_ROOT/runtime.sock).

SELF-DIAGNOSIS
  /status     — overview with Phoenix health, runtime state, frontends, tools
  /diagnose   — deep report: daemon health, modules, errors, security, signalograd
  CLI: harmonia status | harmonia diagnose (same data, from operator shell)

SELF-REPAIR
- (introspect-runtime)          — full diagnostic snapshot including Phoenix health
- (introspect-recent-errors N)  — last N errors with context
- (introspect-libs)             — loaded modules and crash counts
- (%cargo-build-component name) — recompile a single crate
- (%record-lib-crash name msg)  — log a module failure
- ouroboros: write patch, validate with rust-forge, snapshot, reload

CORE LOOP (src/core/loop.lisp)
Tick-based, never crashes, all actions wrapped in handler-case.
Each tick: gateway-poll -> process-prompt -> memory-heartbeat -> harmonic-step -> gateway-flush

MEMORY FIELD (lib/core/memory-field)
Field propagation on the concept graph for dynamical memory recall.
Graph Laplacian L=D-A, spectral eigenmodes (Chladni patterns), 3 attractors (Thomas/Aizawa/Halvorsen).
Hysteresis: basin switching requires sustained coercive energy. Warm-start from Chronicle.
Integration: :observe pushes graph, :attractor-sync steps attractors, :stabilize persists basin.
Recall: memory-layered-recall dispatches to field when available, falls back to substring.

GUARDIAN HEALER (src/core/recovery-cascade.lisp)
Self-healing via LLM-guarded recovery cascade:
  Level 0: Retry (transient errors, 1 attempt)
  Level 1: Fallback (simpler method: field->substring, premium->cheap model)
  Level 2: Pattern detection (repeating errors in ring buffer)
  Level 3: Guardian LLM diagnoses, proposes safe action from whitelist
  Level 4: Restart component via IPC reset
  Level 5: Honest message to user (never 'internal error')
Guardian whitelist: restart-component, reload-config, switch-model,
  skip-feature, clear-memory-cache, reduce-load, report-to-operator.
Guardian CANNOT: mutate vault, change policy, rewrite security, execute code.
%tick-recovery-heartbeat runs every 10 cycles, detects sick components.
All recovery events recorded to Chronicle for learning.

DEBUGGING
- /status and /diagnose for live system state
- tail -f ~A for logs
- state-root ~A for vault.db, config.db
- (%cargo-build-component \"harmonia-runtime\") to recompile"
          (%platform-name)
          (or (%source-root) "unknown")
          (or (%state-root) "unknown")
          (or (%log-path) "unknown")
          (or (%log-path) "unknown")
          (or (%state-root) "unknown")))
