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
(cffi:defcfun ("harmonia_parallel_agents_run_pending_async" %parallel-run-pending-async) :pointer
  (max-parallel :int))
(cffi:defcfun ("harmonia_parallel_agents_task_result" %parallel-task-result) :pointer
  (task-id :long-long))
(cffi:defcfun ("harmonia_parallel_agents_report" %parallel-report) :pointer)
(cffi:defcfun ("harmonia_parallel_agents_last_error" %parallel-last-error) :pointer)
(cffi:defcfun ("harmonia_parallel_agents_free_string" %parallel-free-string) :void
  (ptr :pointer))

;;; --- Tmux CLI Agent CFFI bindings ---

(cffi:defcfun ("harmonia_tmux_spawn" %tmux-spawn) :long-long
  (cli-type :string) (workdir :string) (prompt :string))
(cffi:defcfun ("harmonia_tmux_poll" %tmux-poll) :pointer (id :long-long))
(cffi:defcfun ("harmonia_tmux_kill" %tmux-kill) :int (id :long-long))
(cffi:defcfun ("harmonia_tmux_capture" %tmux-capture) :pointer
  (id :long-long) (history :int))
(cffi:defcfun ("harmonia_tmux_swarm_poll" %tmux-swarm-poll) :pointer)
;;; --- Unified Actor Protocol CFFI bindings ---

(cffi:defcfun ("harmonia_actor_register" %actor-register) :long-long (kind :string))
(cffi:defcfun ("harmonia_actor_heartbeat" %actor-heartbeat) :int (id :long-long) (bytes :unsigned-long-long))
(cffi:defcfun ("harmonia_actor_post" %actor-post) :int (source :long-long) (target :long-long) (payload :string))
(cffi:defcfun ("harmonia_actor_drain" %actor-drain) :pointer)
(cffi:defcfun ("harmonia_actor_state" %actor-state) :pointer (id :long-long))
(cffi:defcfun ("harmonia_actor_list" %actor-list) :pointer)
(cffi:defcfun ("harmonia_actor_deregister" %actor-deregister) :int (id :long-long))
(cffi:defcfun ("harmonia_actor_free_string" %actor-free-string) :void (ptr :pointer))

;;; --- Unified Actor Protocol Lisp wrappers ---

(defun actor-register (kind)
  "Register an actor of KIND (string: gateway, cli-agent, llm-task, chronicle, tailnet).
   Returns actor-id (>= 1) or signals error."
  (let ((id (%actor-register kind)))
    (when (minusp id)
      (error "actor-register failed for kind ~A" kind))
    id))

(defun actor-heartbeat (id &optional (bytes-delta 0))
  "Report progress heartbeat for actor ID."
  (let ((rc (%actor-heartbeat id bytes-delta)))
    (unless (zerop rc)
      (error "actor-heartbeat failed for actor ~D" id))
    t))

(defun actor-post (source target payload-sexp)
  "Post a message to the unified mailbox."
  (let ((rc (%actor-post source target payload-sexp)))
    (unless (zerop rc)
      (error "actor-post failed: source=~D target=~D" source target))
    t))

(defun actor-drain ()
  "Drain all pending messages from the unified actor mailbox. Returns sexp list."
  (let ((ptr (%actor-drain)))
    (if (cffi:null-pointer-p ptr)
        '()
        (let ((raw (unwind-protect
                        (cffi:foreign-string-to-lisp ptr)
                     (%actor-free-string ptr))))
          (handler-case
              (let ((*read-eval* nil))
                (read-from-string raw))
            (error () '()))))))

(defun actor-state (id)
  "Get actor state as parsed sexp."
  (let ((ptr (%actor-state id)))
    (if (cffi:null-pointer-p ptr)
        nil
        (let ((raw (unwind-protect
                        (cffi:foreign-string-to-lisp ptr)
                     (%actor-free-string ptr))))
          (handler-case
              (let ((*read-eval* nil))
                (read-from-string raw))
            (error () nil))))))

(defun actor-list ()
  "List all registered actors as parsed sexp."
  (let ((ptr (%actor-list)))
    (if (cffi:null-pointer-p ptr)
        '()
        (let ((raw (unwind-protect
                        (cffi:foreign-string-to-lisp ptr)
                     (%actor-free-string ptr))))
          (handler-case
              (let ((*read-eval* nil))
                (read-from-string raw))
            (error () '()))))))

(defun actor-deregister (id)
  "Deregister an actor by ID."
  (let ((rc (%actor-deregister id)))
    (zerop rc)))

;;; --- Legacy mailbox drain (delegates to unified) ---

(cffi:defcfun ("harmonia_actor_drain_mailbox" %actor-drain-mailbox) :pointer)

;;; --- Tmux Lisp wrappers ---

(defun tmux-spawn (cli-type workdir prompt)
  "Spawn a tmux CLI agent. Returns agent id (>= 0) or signals error."
  (let ((id (%tmux-spawn cli-type workdir (or prompt ""))))
    (when (minusp id)
      (error "tmux spawn failed: ~A" (parallel-last-error)))
    id))

(defun tmux-poll (id)
  "Poll a tmux agent state. Returns sexp string."
  (%ptr->string (%tmux-poll id)))

(defun tmux-kill (id)
  "Kill a tmux agent."
  (let ((rc (%tmux-kill id)))
    (unless (zerop rc)
      (error "tmux kill failed: ~A" (parallel-last-error)))
    t))

(defun tmux-capture (id &optional (history 200))
  "Capture terminal output of a tmux agent."
  (%ptr->string (%tmux-capture id history)))

(defun tmux-swarm-poll ()
  "Poll all active tmux agents."
  (%ptr->string (%tmux-swarm-poll)))

(defun actor-drain-mailbox ()
  "Drain all pending actor messages from unified actor mailbox. Returns sexp list.
   Delegates to the unified actor-drain which reads from actor-protocol registry."
  (actor-drain))

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

(defun parallel-run-pending-async (&optional (max-parallel 3))
  "Run pending tasks asynchronously — results arrive via unified actor mailbox.
   Returns list of (:task-id T :actor-id A :model M) plists for Lisp-side tracking."
  (let ((ptr (%parallel-run-pending-async max-parallel)))
    (if (cffi:null-pointer-p ptr)
        (error "parallel run pending async failed: ~A" (parallel-last-error))
        (let ((raw (unwind-protect
                        (cffi:foreign-string-to-lisp ptr)
                     (%parallel-free-string ptr))))
          (handler-case
              (let ((*read-eval* nil))
                (read-from-string raw))
            (error () '()))))))

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

(defun %swarm-actor-stall-threshold ()
  "Ticks with zero output delta before killing an actor (progress-based, not time-based)."
  (max 5 (or (ignore-errors (model-policy-actor-stall-threshold)) 50)))

(defun %swarm-spawn-cli-actor (model prompt &optional originating-signal orchestration-ctx)
  "Spawn a CLI subagent as a tmux actor. Returns actor-id. Does NOT block."
  (let* ((cli (%swarm-cli-id model))
         (workdir (or (ignore-errors (config-get-for "conductor" "workdir"))
                      (namestring (user-homedir-pathname))))
         (actor-id (tmux-spawn cli workdir prompt)))
    ;; Register in actor registry
    (let ((record (make-actor-record
                   :id actor-id
                   :model model
                   :prompt prompt
                   :state :running
                   :spawned-at (get-universal-time)
                   :last-heartbeat (get-universal-time)
                   :stall-ticks 0
                   :originating-signal originating-signal
                   :orchestration-context orchestration-ctx
                   :cost-usd 0.0
                   :duration-ms 0)))
      (setf (gethash actor-id (runtime-state-actor-registry *runtime*)) record)
      (push actor-id (runtime-state-actor-pending *runtime*)))
    actor-id))

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

(defun parallel-solve (prompt &key return-structured preferred-models max-subagents
                                   originating-signal orchestration-context)
  "Spawn N subagents with different model/cost profiles, then return best + report.
   CLI models are spawned as non-blocking tmux actors. If ALL models are CLI,
   returns (values :deferred nil nil nil) — results delivered later by %tick-actor-deliver."
  (let* ((n (max 1 (or max-subagents (parallel-get-subagent-count))))
         (chain (or preferred-models
                    (model-escalation-chain prompt (choose-model prompt))))
         (queue (copy-list chain))
         (jobs '())
         (results '())
         (scheduled 0)
         (cli-spawned 0)
         (used-parallel nil)
         (parallel-routed nil))
    (loop while (and queue (< scheduled n)) do
      (let ((m (pop queue)))
        (if (%swarm-cli-model-p m)
            ;; Non-blocking: spawn tmux actor
            (handler-case
                (progn
                  (%swarm-spawn-cli-actor m prompt originating-signal
                                          (or orchestration-context
                                              (list :chain chain
                                                    :prepared-prompt prompt)))
                  (incf scheduled)
                  (incf cli-spawned))
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
            ;; OpenRouter: submit for parallel execution (blocking on join)
            (progn
              (unless parallel-routed
                (harmonic-matrix-route-or-error "orchestrator" "parallel-agents")
                (setf parallel-routed t))
              (push (cons (parallel-submit (format nil "[subagent model=~A] ~A" m prompt) m) m) jobs)
              (setf used-parallel t)
              (incf scheduled)))))
    ;; If ALL scheduled models were CLI, return :deferred
    (when (and (> cli-spawned 0) (null jobs) (null results))
      (return-from parallel-solve
        (if return-structured
            (values :deferred nil nil nil)
            :deferred)))
    ;; Run OpenRouter jobs asynchronously — results arrive via unified actor mailbox.
    ;; Create Lisp-side actor records so %tick-actor-supervisor can track them.
    (when jobs
      (let ((assignments (parallel-run-pending-async (length jobs))))
        (when (listp assignments)
          (dolist (a assignments)
            (let* ((actor-id (getf a :actor-id))
                   (model (getf a :model))
                   (record (make-actor-record
                            :id actor-id
                            :model (or model "openrouter")
                            :prompt prompt
                            :state :running
                            :spawned-at (get-universal-time)
                            :last-heartbeat (get-universal-time)
                            :stall-ticks 0
                            :originating-signal originating-signal
                            :orchestration-context orchestration-context
                            :cost-usd 0.0
                            :duration-ms 0)))
              (setf (gethash actor-id (runtime-state-actor-registry *runtime*)) record)
              (push actor-id (runtime-state-actor-pending *runtime*))))))
      (return-from parallel-solve
        (if return-structured
            (values :deferred nil nil nil)
            :deferred)))
    (setf results (nreverse results))
    (unless results
      (if (> cli-spawned 0)
          ;; CLI actors spawned but no OpenRouter results — defer
          (return-from parallel-solve
            (if return-structured
                (values :deferred nil nil nil)
                :deferred))
          (error "parallel solve failed: no model produced output")))
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
