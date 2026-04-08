;;; swarm.lisp — Port: parallel subagent orchestration via IPC.

(in-package :harmonia)

(defparameter *parallel-subagent-count* 1)
(defparameter *swarm-full-config* nil
  "Full parsed swarm config plist — loaded by parallel-load-policy.")
(defparameter *swarm-config-path*
  (merge-pathnames "../../config/swarm.sexp" *boot-file*))

(declaim (ftype function parallel-load-policy))

(defun %parallel-state-root ()
  (or (config-get-for "parallel-agents-core" "state-root" "global")
      (%tmpdir-state-root)))

(defun %swarm-state-path ()
      (or
      (config-get-for "parallel-agents-core" "policy-path")
      (concatenate 'string (%parallel-state-root) "/swarm.sexp")))

;;; --- Unified Actor Protocol Lisp wrappers (via IPC) ---

(defun actor-register (kind)
  "Register an actor of KIND (string: gateway, cli-agent, llm-task, chronicle, tailnet).
   Returns actor-id (>= 1) or signals error."
  (let ((id (ipc-actor-register kind)))
    (unless (and id (plusp id))
      (error "actor-register failed for kind ~A" kind))
    id))

(defun actor-heartbeat (id &optional (bytes-delta 0))
  "Report progress heartbeat for actor ID."
  (ipc-actor-heartbeat id bytes-delta)
  t)

(defun actor-post (source target payload-sexp)
  "Post a message to the unified mailbox."
  (ipc-actor-post source target payload-sexp)
  t)

(defun actor-drain ()
  "Drain all pending messages from the unified actor mailbox. Returns sexp list."
  (let ((raw (ipc-actor-drain)))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string raw))
      (error () '()))))

(defun actor-state (id)
  "Get actor state as parsed sexp."
  (let ((raw (ipc-actor-state id)))
    (when raw
      (handler-case
          (let ((*read-eval* nil))
            (read-from-string raw))
        (error () nil)))))

(defun actor-list ()
  "List all registered actors as parsed sexp."
  (let ((raw (ipc-actor-list)))
    (handler-case
        (let ((*read-eval* nil))
          (read-from-string raw))
      (error () '()))))

(defun actor-deregister (id)
  "Deregister an actor by ID."
  (let ((reply (ipc-actor-deregister id)))
    (and reply (ipc-reply-ok-p reply))))

;;; --- Legacy mailbox drain (delegates to unified) ---

(defun actor-drain-mailbox ()
  "Drain all pending actor messages from unified actor mailbox. Returns sexp list.
   Delegates to the unified actor-drain which reads from actor-protocol registry."
  (actor-drain))

;;; --- Init ---

(defun init-swarm-port ()
  (let ((reply (ipc-call (%sexp-to-ipc-string
                           '(:component "parallel" :op "init")))))
    (parallel-load-policy)
    (runtime-log *runtime* :parallel-agents-init
                 (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

(defun parallel-last-error ()
  "Parallel agent errors are reported via IPC reply; this returns empty for compat."
  "")

(defun parallel-set-model-price (model in-price out-price)
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "parallel" :op "set-model-price"
                   :model ,model
                   :in-price ,(coerce in-price 'double-float)
                   :out-price ,(coerce out-price 'double-float))))))
    (when (ipc-reply-error-p reply)
      (error "parallel set price failed: ~A" reply))
    t))

(defun parallel-submit (prompt model)
  (let* ((reply (ipc-call
                 (%sexp-to-ipc-string
                  `(:component "parallel" :op "submit"
                    :prompt ,prompt :model ,model))))
         (id (when reply (ipc-extract-u64 reply ":task-id"))))
    (unless (and id (>= id 0))
      (error "parallel submit failed: ~A" (or reply "no reply")))
    id))

(defun parallel-run-pending (&optional (max-parallel 3))
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "parallel" :op "run-pending"
                   :max-parallel ,max-parallel)))))
    (when (ipc-reply-error-p reply)
      (error "parallel run pending failed: ~A" reply))
    t))

(defun parallel-run-pending-async (&optional (max-parallel 3))
  "Run pending tasks asynchronously — results arrive via unified actor mailbox.
   Returns list of (:task-id T :actor-id A :model M) plists for Lisp-side tracking."
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "parallel" :op "run-pending-async"
                   :max-parallel ,max-parallel)))))
    (if (ipc-reply-error-p reply)
        (error "parallel run pending async failed: ~A" reply)
        (let ((val (ipc-extract-value reply)))
          (when val
            (handler-case
                (let ((*read-eval* nil))
                  (read-from-string val))
              (error () '())))))))

(defun parallel-task-result (task-id)
  (let ((reply (ipc-call
                (%sexp-to-ipc-string
                 `(:component "parallel" :op "task-result"
                   :task-id ,task-id)))))
    (or (ipc-extract-value reply)
        (error "parallel task result failed: ~A" (or reply "no reply")))))

(defun parallel-report ()
  (or (ipc-extract-value
       (ipc-call (%sexp-to-ipc-string
                  '(:component "parallel" :op "report"))))
      ""))

;;; --- Pure Lisp policy/config (unchanged) ---

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
    ;; Also load the full config from config/swarm.sexp for prompt templates etc.
    (when (probe-file *swarm-config-path*)
      (handler-case
          (setf *swarm-full-config* (%parallel-read-file *swarm-config-path*))
        (error (_) (declare (ignore _)))))
    (setf *parallel-subagent-count* (max 1 count))
    *parallel-subagent-count*))

(defun %swarm-prompt-template (key &optional default)
  "Get a prompt template.  Looks in config/prompts.sexp :evolution first,
   then falls back to config/swarm.sexp :prompts, then DEFAULT."
  (or (load-prompt :evolution key)
      (getf (getf *swarm-full-config* :prompts) key)
      default))

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
    (handler-case (parallel-save-policy) (error () nil))
    n))

(defun parallel-get-subagent-count ()
  *parallel-subagent-count*)

