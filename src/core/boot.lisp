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
(load (%core-path "../harmony/scorer.lisp"))
(%ensure-ffi-deps)
(load (%core-path "../backends/openrouter.lisp"))
(load (%core-path "../backends/git-ops.lisp"))
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
      (setf (runtime-state-rewrite-count *runtime*) 0))
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
  (init-native-backends)
  (init-git-ops-backend)
  (format t "[harmonia] bootstrap complete (~D tools registered).~%"
          (hash-table-count (runtime-state-tools *runtime*)))
  (when run-loop
    (run-loop :runtime *runtime* :max-cycles max-cycles :sleep-seconds sleep-seconds))
  *runtime*)
