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

(defun %swarm-starts-with-p (text prefix)
  (let ((s (or text ""))
        (p (or prefix "")))
    (and (>= (length s) (length p))
         (string-equal p s :end2 (length p)))))

(defun %swarm-cli-model-p (model)
  (%swarm-starts-with-p model "cli:"))

(defun %swarm-cli-id (model)
  (if (%swarm-cli-model-p model) (subseq model 4) model))

(defun %swarm-cli-timeout-seconds ()
  (max 5 (or (ignore-errors (model-policy-cli-timeout-seconds)) 90)))

(defun %swarm-read-stream-text (stream)
  (if stream
      (with-output-to-string (s)
        (loop for line = (read-line stream nil nil)
              while line
              do (write-line line s)))
      ""))

(defun %swarm-process-running-p (proc)
  (string= (string (sb-ext:process-status proc)) "RUNNING"))

(defun %swarm-run-cli-subagent (model prompt)
  (let* ((cli (%swarm-cli-id model))
         (command (cond
                    ((string= cli "claude-code") "claude")
                    ((string= cli "codex") "codex")
                    (t cli)))
         (args (cond
                 ((string= cli "claude-code")
                  (list "--dangerously-skip-permissions" "-p" (or prompt "")))
                 ((string= cli "codex")
                  (list "exec" "--full-auto" (or prompt "")))
                 (t (list (or prompt "")))))
         (timeout-seconds (%swarm-cli-timeout-seconds))
         (started-at (get-internal-real-time))
         (proc (sb-ext:run-program command args
                                   :output :stream
                                   :error :output
                                   :search t
                                   :wait nil)))
    (loop while (%swarm-process-running-p proc) do
      (let ((elapsed (/ (- (get-internal-real-time) started-at)
                        internal-time-units-per-second)))
        (when (>= elapsed timeout-seconds)
          (ignore-errors (sb-ext:process-kill proc 15))
          (sleep 0.2)
          (when (%swarm-process-running-p proc)
            (ignore-errors (sb-ext:process-kill proc 9)))
          (error "cli subagent ~A timed out after ~Ds" cli timeout-seconds)))
      (sleep 0.1))
    (let* ((stream (sb-ext:process-output proc))
           (output (%swarm-read-stream-text stream))
           (exit-code (sb-ext:process-exit-code proc)))
      (ignore-errors (when stream (close stream)))
      (unless (and exit-code (zerop exit-code))
        (error "cli subagent ~A failed: ~A" cli output))
      (let ((trimmed (string-trim '(#\Space #\Newline #\Tab) output)))
        (unless (> (length trimmed) 0)
          (error "cli subagent ~A returned empty output" cli))
        trimmed))))

(defun %swarm-clean-text (text)
  (string-trim '(#\Space #\Newline #\Tab #\Return) (or text "")))

(defun %swarm-parse-task-result (raw &optional fallback-model)
  (let ((trimmed (%swarm-clean-text raw)))
    (handler-case
        (let* ((*read-eval* nil)
               (sexp (read-from-string trimmed nil nil)))
          (if (and (listp sexp) (getf sexp :model))
              (let* ((model (or (getf sexp :model) fallback-model))
                     (success (not (null (getf sexp :success))))
                     (response (%swarm-clean-text (or (getf sexp :response) "")))
                     (latency (or (getf sexp :latency-ms) 0))
                     (cost (or (getf sexp :cost-usd) 0.0))
                     (error-text (%swarm-clean-text (or (getf sexp :error) ""))))
                (list :model model
                      :text response
                      :success success
                      :latency-ms latency
                      :cost-usd cost
                      :error error-text))
              (list :model fallback-model
                    :text trimmed
                    :success (> (length trimmed) 0)
                    :latency-ms 0
                    :cost-usd 0.0
                    :error "")))
      (error (_)
        (declare (ignore _))
        (list :model fallback-model
              :text trimmed
              :success (> (length trimmed) 0)
              :latency-ms 0
              :cost-usd 0.0
              :error "")))))

(defun parallel-solve (prompt &key return-structured preferred-models max-subagents)
  "Spawn N subagents with different model/cost profiles, then return best + report."
  (let* ((n (max 1 (or max-subagents (parallel-get-subagent-count))))
         (chain (or preferred-models
                    (model-escalation-chain prompt (choose-model prompt))))
         (queue (copy-list chain))
         (jobs '())
         (results '())
         (scheduled 0)
         (used-parallel nil)
         (parallel-routed nil))
    (loop while (and queue (< scheduled n)) do
      (let ((m (pop queue)))
        (if (%swarm-cli-model-p m)
            (handler-case
                (progn
                  (push (list :model m
                              :text (%swarm-run-cli-subagent m prompt)
                              :success t
                              :latency-ms 0
                              :cost-usd 0.0
                              :error "")
                        results)
                  (incf scheduled))
              (error (e)
                (let ((msg (princ-to-string e)))
                  (ignore-errors (model-policy-mark-cli-cooloff (%swarm-cli-id m)))
                  (when (model-policy-cli-quota-exceeded-p msg)
                    (ignore-errors (model-policy-mark-cli-cooloff (%swarm-cli-id m))))
                  (ignore-errors
                    (model-policy-record-outcome
                     :model m
                     :success nil
                     :latency-ms 0
                     :harmony-score 0.0
                     :cost-usd 0.0)))))
            (progn
              (unless parallel-routed
                (harmonic-matrix-route-or-error "orchestrator" "parallel-agents")
                (setf parallel-routed t))
              (push (cons (parallel-submit (format nil "[subagent model=~A] ~A" m prompt) m) m) jobs)
              (setf used-parallel t)
              (incf scheduled)))))
    (when jobs
      (parallel-run-pending (length jobs))
      (dolist (job jobs)
        (push (%swarm-parse-task-result (parallel-task-result (car job)) (cdr job)) results)))
    (setf results (nreverse results))
    (unless results
      (error "parallel solve failed: no model produced output"))
    (let ((usable-results '()))
      (dolist (entry results)
        (let* ((model (or (getf entry :model) "unknown"))
               (text (%swarm-clean-text (getf entry :text)))
               (success (and (getf entry :success) (> (length text) 0)))
               (latency (or (getf entry :latency-ms) 0))
               (base-cost (or (getf entry :cost-usd) 0.0))
               (cost (if (> base-cost 0.0)
                         base-cost
                         (model-policy-estimate-cost-usd model prompt text)))
               (score (if success (harmonic-score prompt text) 0.0)))
          (setf (getf entry :model) model)
          (setf (getf entry :text) text)
          (setf (getf entry :success) success)
          (setf (getf entry :latency-ms) latency)
          (setf (getf entry :cost-usd) cost)
          (setf (getf entry :score) score)
          (ignore-errors
            (model-policy-record-outcome
             :model model
             :success success
             :latency-ms latency
             :harmony-score score
             :cost-usd cost))
          (when success
            (push entry usable-results))))
      (setf usable-results (nreverse usable-results))
      (unless usable-results
        (error "parallel solve failed: all model attempts failed"))
      (let* ((best-entry (car (sort (copy-list usable-results) #'> :key (lambda (e) (getf e :score)))))
             (best (getf best-entry :text))
             (rep (if used-parallel
                      (or (ignore-errors (parallel-report)) "parallel-report-unavailable")
                      "direct-cli")))
        (when used-parallel
          (harmonic-matrix-observe-route "orchestrator" "parallel-agents" t 1)
          (harmonic-matrix-observe-route "parallel-agents" "memory" t 1))
        (if return-structured
            (values best rep best-entry usable-results)
            (format nil "PARALLEL_BEST=~A~%PARALLEL_REPORT=~A" best rep))))))
