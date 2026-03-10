;;; swarm.lisp — Port: parallel subagent orchestration via parallel-agents CFFI.

(in-package :harmonia)

(defparameter *parallel-agents-lib* nil)
(defparameter *parallel-subagent-count* 1)
(defparameter *swarm-config-path*
  (merge-pathnames "../../config/swarm.sexp" *boot-file*))

(declaim (ftype function parallel-load-policy))

(defun %parallel-state-root ()
  (or (config-get-for "parallel-agents-core" "state-root" "global")
      (let ((base (or (sb-ext:posix-getenv "TMPDIR")
                      (namestring (user-homedir-pathname)))))
        (concatenate 'string (string-right-trim "/" base) "/harmonia"))))

(defun %swarm-state-path ()
      (or
      (config-get-for "parallel-agents-core" "policy-path")
      (concatenate 'string (%parallel-state-root) "/swarm.sexp")))

(cffi:defcfun ("harmonia_parallel_agents_init" %parallel-init) :int)
(cffi:defcfun ("harmonia_parallel_agents_set_model_price" %parallel-set-price) :int
  (model :string) (in-price :double) (out-price :double))
(cffi:defcfun ("harmonia_parallel_agents_submit" %parallel-submit) :long-long
  (prompt :string) (model :string))
(cffi:defcfun ("harmonia_parallel_agents_run_pending" %parallel-run-pending) :int
  (max-parallel :int))
(cffi:defcfun ("harmonia_parallel_agents_task_result" %parallel-task-result) :pointer
  (task-id :long-long))
(cffi:defcfun ("harmonia_parallel_agents_report" %parallel-report) :pointer)
(cffi:defcfun ("harmonia_parallel_agents_last_error" %parallel-last-error) :pointer)
(cffi:defcfun ("harmonia_parallel_agents_free_string" %parallel-free-string) :void
  (ptr :pointer))

(defun init-swarm-port ()
  (ensure-cffi)
  (setf *parallel-agents-lib*
        (cffi:load-foreign-library (%release-lib-path "libharmonia_parallel_agents.dylib")))
  (let ((rc (%parallel-init)))
    (parallel-load-policy)
    (runtime-log *runtime* :parallel-agents-init (list :status rc))
    (zerop rc)))

(defun parallel-last-error ()
  (let ((ptr (%parallel-last-error)))
    (if (cffi:null-pointer-p ptr)
        ""
        (unwind-protect
             (cffi:foreign-string-to-lisp ptr)
          (%parallel-free-string ptr)))))

(defun parallel-set-model-price (model in-price out-price)
  (let ((rc (%parallel-set-price model (coerce in-price 'double-float) (coerce out-price 'double-float))))
    (unless (zerop rc)
      (error "parallel set price failed: ~A" (parallel-last-error)))
    t))

(defun parallel-submit (prompt model)
  (let ((id (%parallel-submit prompt model)))
    (when (minusp id)
      (error "parallel submit failed: ~A" (parallel-last-error)))
    id))

(defun parallel-run-pending (&optional (max-parallel 3))
  (let ((rc (%parallel-run-pending max-parallel)))
    (unless (zerop rc)
      (error "parallel run pending failed: ~A" (parallel-last-error)))
    t))

(defun %ptr->string (ptr)
  (if (cffi:null-pointer-p ptr)
      nil
      (unwind-protect
           (cffi:foreign-string-to-lisp ptr)
        (%parallel-free-string ptr))))

(defun parallel-task-result (task-id)
  (let ((ptr (%parallel-task-result task-id)))
    (or (%ptr->string ptr)
        (error "parallel task result failed: ~A" (parallel-last-error)))))

(defun parallel-report ()
  (let ((ptr (%parallel-report)))
    (or (%ptr->string ptr)
        (error "parallel report failed: ~A" (parallel-last-error)))))

(defun %parallel-read-file (path)
  (with-open-file (in path :direction :input)
    (let ((*read-eval* nil))
      (read in nil nil))))

(defun parallel-load-policy ()
  (let* ((state-path (%swarm-state-path))
         (source (cond
                   ((probe-file state-path) (%parallel-read-file state-path))
                   ((probe-file *swarm-config-path*) (%parallel-read-file *swarm-config-path*))
                   (t '(:subagent-count 1))))
         (count (or (getf source :subagent-count) 1)))
    (setf *parallel-subagent-count* (max 1 count))
    *parallel-subagent-count*))

(defun parallel-save-policy ()
  (let ((path (%swarm-state-path)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 (list :subagent-count *parallel-subagent-count*) out)
        (terpri out)))
    path))

(defun parallel-set-subagent-count (count)
  (let ((n (max 1 count)))
    (setf *parallel-subagent-count* n)
    (ignore-errors (parallel-save-policy))
    n))

(defun parallel-get-subagent-count ()
  *parallel-subagent-count*)

(defun parallel-solve (prompt)
  "Spawn N subagents with different model/cost profiles, then return best + report."
  (harmonic-matrix-route-or-error "orchestrator" "parallel-agents")
  (let* ((n (parallel-get-subagent-count))
         (chain (model-escalation-chain prompt (%pick-heuristic-model prompt)))
         (models (subseq chain 0 (min n (length chain))))
         (ids (mapcar (lambda (m)
                        (parallel-submit (format nil "[subagent model=~A] ~A" m prompt) m))
                      models)))
    (parallel-run-pending n)
    (let* ((results (mapcar #'parallel-task-result ids))
           (best (car (sort (copy-seq results) #'>
                            :key (lambda (r) (harmonic-score prompt r)))))
           (rep (parallel-report)))
      (harmonic-matrix-observe-route "orchestrator" "parallel-agents" t 1)
      (harmonic-matrix-observe-route "parallel-agents" "memory" t 1)
      (format nil "PARALLEL_BEST=~A~%PARALLEL_REPORT=~A" best rep))))
