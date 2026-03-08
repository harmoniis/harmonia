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
           :gateway-list-frontends
           :gateway-frontend-status
           :gateway-list-channels
           :gateway-shutdown
           :register-configured-frontends
           :search-web
           :tool-runtime-list
           :router-healthcheck
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
           :*runtime*))

(in-package :harmonia)

(defparameter *runtime* nil)
(defparameter *boot-file* *load-truename*)

(defun %core-path (name)
  (merge-pathnames name (make-pathname :name nil :type nil :defaults *boot-file*)))

(defun %environment ()
  (or (sb-ext:posix-getenv "HARMONIA_ENV") "test"))

(defun %enforce-genesis-safety ()
  (let ((env (string-downcase (%environment))))
    (when (string= env "prod")
      (unless (string= (or (sb-ext:posix-getenv "HARMONIA_ALLOW_PROD_GENESIS") "") "1")
        (error "Production genesis is blocked. Set HARMONIA_ALLOW_PROD_GENESIS=1 explicitly to override.")))))

(defun %ensure-ffi-deps ()
  (load #P"~/quicklisp/setup.lisp")
  (let* ((ql-package (find-package :ql))
         (quickload (and ql-package (find-symbol "QUICKLOAD" ql-package))))
    (unless quickload
      (error "Quicklisp did not provide QL:QUICKLOAD"))
    (funcall quickload :cffi)))

(load (%core-path "state.lisp"))
(load (%core-path "tools.lisp"))
(load (%core-path "../dna/dna.lisp"))
(load (%core-path "../memory/store.lisp"))
(load (%core-path "conditions.lisp"))
(load (%core-path "../harmony/scorer.lisp"))
(load (%core-path "harmony-policy.lisp"))
(load (%core-path "../orchestrator/prompt-assembly.lisp"))
(load (%core-path "model-policy.lisp"))
(load (%core-path "harmonic-machine.lisp"))
(load (%core-path "evolution-versioning.lisp"))
(%ensure-ffi-deps)
  (load (%core-path "../ports/vault.lisp"))
  (load (%core-path "../ports/store.lisp"))
  (load (%core-path "../ports/router.lisp"))
  (load (%core-path "../ports/lineage.lisp"))
  (load (%core-path "../ports/matrix.lisp"))
  (load (%core-path "../ports/admin-intent.lisp"))
  (load (%core-path "../ports/tool-runtime.lisp"))
(load (%core-path "../ports/baseband.lisp"))
(load (%core-path "../ports/swarm.lisp"))
(load (%core-path "../ports/evolution.lisp"))
(load (%core-path "../orchestrator/conductor.lisp"))
(load (%core-path "rewrite.lisp"))
(load (%core-path "loop.lisp"))

(defun reset-test-genesis ()
  (let ((env (string-downcase (%environment))))
    (unless (string= env "test")
      (error "reset-test-genesis is only allowed in HARMONIA_ENV=test."))
    (when *runtime*
      (setf (runtime-state-events *runtime*) '())
      (setf (runtime-state-prompt-queue *runtime*) '())
      (setf (runtime-state-responses *runtime*) '())
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
    (error "DNA validation failed; refusing to start."))
  (register-default-tools *runtime*)
  (memory-seed-soul-from-dna)
  (init-evolution-versioning)
  (init-vault-port)
  (init-admin-intent-port)
  (init-store-port)
  (harmony-policy-load)
  (model-policy-load)
  (init-router-port)
  (init-lineage-port)
  (bootstrap-harmonic-matrix)
  (init-tool-runtime-port)
  (init-baseband-port)
  (register-configured-frontends)
  (init-swarm-port)
  (init-evolution-port)
  (format t "[harmonia] bootstrap complete (~D tools registered).~%"
          (hash-table-count (runtime-state-tools *runtime*)))
  (when run-loop
    (run-loop :runtime *runtime* :max-cycles max-cycles :sleep-seconds sleep-seconds))
  *runtime*)
