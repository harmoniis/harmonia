;;; boot.lisp — Bootstrap: load runtime and start Harmonia.

(in-package :cl-user)

(defpackage :harmonia
  (:use :cl)
  (:export :start
           :stop
           :tick
           :run-loop
           :register-default-tools
           :tool-status
           :feed-prompt
           :run-prompt
           :run-self-push-test
           :vault-set-secret
           :vault-has-secret-p
           :vault-list-symbols
           :config-set
           :config-get
           :config-list
           :memory-recent
           :memory-layered-recall
           :memory-bootstrap-context
           :memory-semantic-recall-block
           :memory-maybe-journal-yesterday
           :memory-map-sexp
           :harmonic-state-step
           :gateway-version
           :gateway-healthcheck
           :gateway-register
           :gateway-unregister
           :gateway-poll
           :gateway-send
           :baseband-poll
           :baseband-send
           :gateway-list-frontends
           :gateway-frontend-status
           :gateway-list-channels
           :baseband-channel-status
           :baseband-list-channels
           :gateway-shutdown
           :register-configured-frontends
           :search-web
           :tool-runtime-list
           :router-healthcheck
           :backend-list-backends
           :backend-backend-status
           :parallel-set-model-price
           :parallel-submit
           :parallel-run-pending
           :parallel-task-result
           :parallel-report
           :parallel-solve
           :parallel-set-subagent-count
           :parallel-get-subagent-count
           :parallel-load-policy
           :parallel-save-policy
           :model-policy-get
           :model-policy-set-weight
           :model-policy-upsert-profile
           :model-policy-load
           :model-policy-save
           :model-feature-params
           :swarm-evolve-scores
           :harmony-policy-get
           :harmony-policy-load
           :harmony-policy-save
           :harmony-policy-set
           :harmonic-matrix-set-tool-enabled
           :harmonic-matrix-set-tool
           :harmonic-matrix-set-store
           :harmonic-matrix-store-config
           :harmonic-matrix-set-node
           :harmonic-matrix-set-edge
           :harmonic-matrix-route-check
           :harmonic-matrix-route-defaults
           :harmonic-matrix-set-route-defaults
           :harmonic-matrix-current-topology
           :harmonic-matrix-save-topology
           :harmonic-matrix-load-topology
           :harmonic-matrix-reset-defaults
           :harmonic-matrix-log-event
           :harmonic-matrix-route-timeseries
           :harmonic-matrix-time-report
           :harmonic-matrix-report
           :whisper-transcribe
           :elevenlabs-tts-to-file
           :chronicle-query
           :chronicle-harmony-summary
           :chronicle-delegation-report
           :chronicle-cost-report
           :chronicle-full-digest
           :chronicle-harmonic-history
           :chronicle-memory-history
           :chronicle-delegation-history
           :chronicle-dashboard-json
           :chronicle-graph-traverse
           :chronicle-graph-bridges
           :chronicle-graph-domains
           :chronicle-graph-central
           :chronicle-graph-evolution
           :chronicle-record-graph-snapshot
           :chronicle-gc
           :chronicle-gc-status
           :signalograd-status
           :signalograd-snapshot
           :signalograd-current-projection
           :evolution-mode
           :evolution-set-mode
           :evolution-prepare
           :evolution-execute
           :evolution-rollback
           :evolution-current-version
           :evolution-list-versions
           :evolution-load-latest-snapshot
           :evolution-snapshot-latest
           :reset-test-genesis
           :trace-start
           :trace-end
           :trace-event
           :with-trace
           :trace-flush
           :trace-shutdown
           :*runtime*))

(in-package :harmonia)

(defparameter *runtime* nil)
(defparameter *boot-file* *load-truename*)

;;; ─── Early env helper (before introspection.lisp loads) ──────────────

(unless (fboundp '%boot-env)
  (defun %boot-env (name &optional default)
    "Read an environment variable. Early definition; introspection.lisp provides the canonical one."
    (let ((val (sb-ext:posix-getenv name)))
      (if (and val (plusp (length val))) val default))))

;;; ─── Logging ──────────────────────────────────────────────────────────

(defparameter *log-level*
  (let ((env (or (%boot-env "HARMONIA_LOG_LEVEL") "info")))
    (cond
      ((string-equal env "debug") :debug)
      ((string-equal env "warn")  :warn)
      ((string-equal env "error") :error)
      (t :info)))
  "Log verbosity: :debug, :info, :warn, :error")

(defun %log-level-rank (level)
  (case level (:debug 0) (:info 1) (:warn 2) (:error 3) (t 1)))

(defun %log (level tag message &rest args)
  "Structured log output: [LEVEL] [tag] message"
  (when (>= (%log-level-rank level) (%log-level-rank *log-level*))
    (let ((prefix (case level
                    (:debug "DEBUG")
                    (:info  "INFO")
                    (:warn  "WARN")
                    (:error "ERROR")
                    (t      "INFO")))
          (msg (if args (apply #'format nil message args) message)))
      (format *error-output* "[~A] [~A] ~A~%" prefix tag msg)
      (force-output *error-output*))))

;;; ─── Helpers ──────────────────────────────────────────────────────────

(defun %core-path (name)
  (merge-pathnames name (make-pathname :name nil :type nil :defaults *boot-file*)))

(defun %environment ()
  (or (%boot-env "HARMONIA_ENV") "test"))

(defun %enforce-genesis-safety ()
  (let ((env (string-downcase (%environment))))
    (when (string= env "prod")
      (unless (string= (or (%boot-env "HARMONIA_ALLOW_PROD_GENESIS") "") "1")
        (error "Production genesis is blocked. Set HARMONIA_ALLOW_PROD_GENESIS=1 explicitly to override.")))))

;;; ─── Module loading (style-warnings suppressed) ───────────────────────

(defun %load-module (path &optional label)
  "Load a Lisp file with style-warnings muffled."
  (let ((name (or label (pathname-name (pathname path)))))
    (%log :debug "boot" "Loading ~A..." name)
    (handler-bind ((style-warning #'muffle-warning))
      (load path))))

(%load-module (%core-path "state.lisp"))
(%load-module (%core-path "tools.lisp"))
(%load-module (%core-path "../dna/dna.lisp") "dna")
(%load-module (%core-path "../memory/store.lisp") "memory")
(%load-module (%core-path "conditions.lisp"))
(%load-module (%core-path "introspection.lisp"))
(%load-module (%core-path "recovery-cascade.lisp") "recovery-cascade")
(%load-module (%core-path "sexp-eval.lisp") "sexp-eval")
(%load-module (%core-path "supervision-state.lisp") "supervision-state")
(%load-module (%core-path "../harmony/scorer.lisp") "harmony-scorer")
(%load-module (%core-path "harmony-policy.lisp"))
(%load-module (%core-path "signalograd.lisp") "signalograd")
(%load-module (%core-path "../orchestrator/prompt-assembly.lisp") "prompt-assembly")
(load-prompts-config)
(load-security-patterns-config)
(%load-module (%core-path "model-policy.lisp"))
(%load-module (%core-path "harmonic-machine.lisp"))
(%load-module (%core-path "evolution-versioning.lisp"))
(%load-module (%core-path "../ports/ipc-client.lisp") "port/ipc-client")
(%load-module (%core-path "../ports/ipc-ports.lisp") "port/ipc-ports")
(%load-module (%core-path "../ports/observability.lisp") "port/observability")
(%load-module (%core-path "../ports/vault.lisp") "port/vault")
(%load-module (%core-path "../ports/store.lisp") "port/store")
(%load-module (%core-path "../ports/router.lisp") "port/router")
(%load-module (%core-path "../ports/lineage.lisp") "port/lineage")
(%load-module (%core-path "../ports/matrix.lisp") "port/matrix")
(%load-module (%core-path "../ports/admin-intent.lisp") "port/admin-intent")
(%load-module (%core-path "../ports/tool-runtime.lisp") "port/tool-runtime")
(%load-module (%core-path "../ports/baseband.lisp") "port/baseband")
(%load-module (%core-path "../ports/swarm.lisp") "port/swarm")
(%load-module (%core-path "../ports/evolution.lisp") "port/evolution")
(%load-module (%core-path "../ports/chronicle.lisp") "port/chronicle")
(%load-module (%core-path "../ports/signalograd.lisp") "port/signalograd")
(%load-module (%core-path "../ports/memory-field.lisp") "port/memory-field")
(%load-module (%core-path "supervisor.lisp") "supervisor")
(%load-module (%core-path "system-commands.lisp") "system-commands")
(%load-module (%core-path "../orchestrator/conductor.lisp") "conductor")
(%load-module (%core-path "rewrite.lisp"))
(%load-module (%core-path "actors.lisp"))
(%load-module (%core-path "loop.lisp"))

(%log :info "boot" "All modules loaded.")

(defun %seed-rust-config ()
  "Write prompt templates, keywords, and model capabilities to config-store for Rust."
  (handler-case
      (progn
        ;; Grok verification prompt template
        (let ((v (load-prompt :evolution :grok-verification)))
          (when v (config-set-for "conductor" "grok-verification" v "prompts")))
        ;; Truth-seeking keywords as pipe-separated string
        (let ((kw (load-security-pattern :truth-seeking-keywords)))
          (when kw (config-set-for "conductor" "truth-seeking-keywords"
                                   (format nil "~{~A~^|~}" kw) "prompts")))
        ;; Preferred truth-seeking model
        (let ((models (remove-if-not
                       (lambda (p) (getf (getf p :features) :truth-seeking))
                       *model-profiles*)))
          (when models
            (config-set-for "conductor" "truth-seeking-model"
                            (getf (first models) :id) "prompts")))
        ;; Model native-tools manifests (serialized sexp for Rust to parse)
        (dolist (profile *model-profiles*)
          (let ((id (getf profile :id))
                (tools (getf profile :native-tools)))
            (when tools
              (config-set-for "conductor" id
                              (format nil "~S" tools) "model-capabilities"))))
        ;; Seed bootstrap env values so post-init code can use config-get-for
        (let ((env (%environment)))
          (when env (config-set-for "conductor" "env" env "global")))
        (when *log-level*
          (config-set-for "conductor" "log-level"
                          (string-downcase (symbol-name *log-level*)) "global"))
        (%log :debug "boot" "Rust config seeded."))
    (error (e)
      (%log :warn "boot" "Failed to seed Rust config: ~A" e))))

(defun reset-test-genesis ()
  (let ((env (string-downcase (%environment))))
    (unless (string= env "test")
      (error "reset-test-genesis is only allowed in HARMONIA_ENV=test."))
    (when *runtime*
      (setf (runtime-state-events *runtime*) '())
      (setf (runtime-state-prompt-queue *runtime*) '())
      (setf (runtime-state-responses *runtime*) '())
      (setf (runtime-state-response-seq *runtime*) 0)
      (setf (runtime-state-presentation-feedback *runtime*) '())
      (setf (runtime-state-last-response-telemetry *runtime*) '())
      (setf (runtime-state-cycle *runtime*) 0)
      (setf (runtime-state-rewrite-count *runtime*) 0)
      (setf (runtime-state-harmonic-phase *runtime*) :observe)
      (setf (runtime-state-harmonic-context *runtime*) '())
      (setf (runtime-state-harmonic-x *runtime*) 0.5)
      (setf (runtime-state-harmonic-r *runtime*) 3.45)
      (setf (runtime-state-lorenz-x *runtime*) 0.1)
      (setf (runtime-state-lorenz-y *runtime*) 0.0)
      (setf (runtime-state-lorenz-z *runtime*) 0.0))
    (memory-reset)))

(defun run-prompt (prompt &key (max-cycles 4))
  (feed-prompt prompt)
  (run-loop :runtime *runtime* :max-cycles max-cycles :sleep-seconds 0.05)
  (first (runtime-state-responses *runtime*)))

(defun run-self-push-test (repo branch)
  (let ((prompt (format nil "self-push-test repo=~A branch=~A" repo branch)))
    (run-prompt prompt :max-cycles 2)))

(defun start (&key (run-loop t) (max-cycles nil) (sleep-seconds 1.0))
  "Initialize runtime and optionally enter the main loop."
  (%enforce-genesis-safety)
  (setf *runtime* (make-runtime-state))
  (setf (runtime-state-environment *runtime*) (%environment))
  (unless (dna-valid-p)
    (%log :error "boot" "DNA validation failed; refusing to start.")
    (error "DNA validation failed; refusing to start."))
  (%log :info "boot" "Registering tools...")
  (register-default-tools *runtime*)
  (memory-seed-soul-from-dna)
  (%log :info "boot" "Initializing subsystems...")
  (init-vault-port)
  (%log :info "vault" "Initialized.")
  (init-admin-intent-port)
  (init-store-port)
  (%log :info "config-store" "Initialized.")
  (init-evolution-versioning)
  (harmony-policy-load)
  (model-policy-load)
  (%seed-rust-config)
  (init-router-port)
  (%log :info "router" "Initialized.")
  (init-lineage-port)
  (bootstrap-harmonic-matrix)
  (%log :info "matrix" "Initialized.")
  (init-tool-runtime-port)
  (init-baseband-port)
  (%log :info "gateway" "Initialized.")
  (register-configured-frontends)
  (init-swarm-port)
  (init-evolution-port)
  (init-chronicle-port)
  (handler-case (init-observability-port)
    (error (e)
      (%log :warn "boot" "Observability init failed (non-fatal): ~A" e)))
  ;; Load persistent memories from Chronicle.
  (ignore-errors
    (when (fboundp '%load-memories-from-chronicle)
      (%load-memories-from-chronicle)))
  ;; Always ensure genesis memories exist (idempotent — dedup by content hash).
  (memory-seed-soul-from-dna)
  (init-signalograd-port)
  (ignore-errors
    (signalograd-restore-for-current-evolution :runtime *runtime*))
  ;; Memory-field: initialize port, push graph, warm-start basin.
  (ignore-errors (init-memory-field-port))
  (ignore-errors (memory-field-warm-start-from-chronicle))
  (%log :info "chronicle" "Initialized.")
  (%log :info "signalograd" "Initialized.")
  (%log :info "memory-field" "Initialized (basin: ~A, memories: ~D)."
        (if (and (fboundp 'memory-field-port-ready-p)
                 (funcall 'memory-field-port-ready-p))
            "active" "unavailable")
        (hash-table-count *memory-store*))
  (%log :info "boot" "Bootstrap complete (~D tools registered)."
        (hash-table-count (runtime-state-tools *runtime*)))
  ;; Handle SIGTERM for graceful shutdown (Phoenix sends this on stop)
  (sb-sys:enable-interrupt sb-unix:sigterm
    (lambda (signal context info)
      (declare (ignore signal context info))
      (%log :info "boot" "Received SIGTERM, shutting down gracefully.")
      (stop *runtime*)))
  (when run-loop
    (%log :info "boot" "Entering main loop (env=~A)." (%environment))
    (run-loop :runtime *runtime* :max-cycles max-cycles :sleep-seconds sleep-seconds))
  *runtime*)
