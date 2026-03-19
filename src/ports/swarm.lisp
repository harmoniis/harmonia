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

;;; --- Tmux Lisp wrappers (via IPC component dispatch) ---

(defun tmux-spawn (cli-type workdir prompt)
  "Spawn a tmux CLI agent. Returns agent id (>= 0) or signals error."
  (let* ((reply (ipc-call
                 (format nil "(:component \"tmux\" :op \"spawn\" :cli-type \"~A\" :workdir \"~A\" :prompt \"~A\")"
                         (sexp-escape-lisp cli-type)
                         (sexp-escape-lisp workdir)
                         (sexp-escape-lisp (or prompt "")))))
         (id (when reply (ipc-extract-u64 reply ":id"))))
    (unless (and id (>= id 0))
      (error "tmux spawn failed: ~A" (or reply "no reply")))
    id))

(defun tmux-poll (id)
  "Poll a tmux agent state. Returns sexp string."
  (ipc-extract-value
   (ipc-call (format nil "(:component \"tmux\" :op \"poll\" :id ~D)" id))))

(defun tmux-kill (id)
  "Kill a tmux agent."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"kill\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux kill failed: ~A" reply))
    t))

(defun tmux-capture (id &optional (history 200))
  "Capture terminal output of a tmux agent."
  (ipc-extract-value
   (ipc-call (format nil "(:component \"tmux\" :op \"capture\" :id ~D :history ~D)" id history))))

(defun tmux-swarm-poll ()
  "Poll all active tmux agents."
  (ipc-extract-value
   (ipc-call "(:component \"tmux\" :op \"swarm-poll\")")))

(defun tmux-send-input (id input)
  "Send text input followed by Enter to a tmux CLI agent."
  (let ((reply (ipc-call
                (format nil "(:component \"tmux\" :op \"send\" :id ~D :input \"~A\")"
                        id (sexp-escape-lisp (or input ""))))))
    (when (ipc-reply-error-p reply)
      (error "tmux send failed: ~A" reply))
    t))

(defun tmux-send-key (id key)
  "Send a special key (Enter, Tab, Escape, Up, Down, C-c, etc.) to a tmux agent."
  (let ((reply (ipc-call
                (format nil "(:component \"tmux\" :op \"send-key\" :id ~D :key \"~A\")"
                        id (sexp-escape-lisp (or key ""))))))
    (when (ipc-reply-error-p reply)
      (error "tmux send-key failed: ~A" reply))
    t))

(defun tmux-approve (id)
  "Approve a permission prompt on a tmux CLI agent."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"approve\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux approve failed: ~A" reply))
    t))

(defun tmux-deny (id)
  "Deny a permission prompt on a tmux CLI agent."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"deny\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux deny failed: ~A" reply))
    t))

(defun tmux-confirm-yes (id)
  "Confirm yes on a tmux CLI agent confirmation prompt."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"confirm-yes\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux confirm-yes failed: ~A" reply))
    t))

(defun tmux-confirm-no (id)
  "Confirm no on a tmux CLI agent confirmation prompt."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"confirm-no\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux confirm-no failed: ~A" reply))
    t))

(defun tmux-select-option (id index)
  "Select option by INDEX (0-based) on a tmux CLI agent selection menu."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"select\" :id ~D :index ~D)" id index))))
    (when (ipc-reply-error-p reply)
      (error "tmux select failed: ~A" reply))
    t))

(defun tmux-interrupt (id)
  "Send Ctrl+C interrupt to a tmux CLI agent."
  (let ((reply (ipc-call (format nil "(:component \"tmux\" :op \"interrupt\" :id ~D)" id))))
    (when (ipc-reply-error-p reply)
      (error "tmux interrupt failed: ~A" reply))
    t))

;;; --- Init ---

(defun init-swarm-port ()
  (let ((reply (ipc-call "(:component \"parallel\" :op \"init\")")))
    (parallel-load-policy)
    (runtime-log *runtime* :parallel-agents-init
                 (list :status (if (ipc-reply-ok-p reply) 0 -1)))
    (ipc-reply-ok-p reply)))

(defun parallel-last-error ()
  "Parallel agent errors are reported via IPC reply; this returns empty for compat."
  "")

(defun parallel-set-model-price (model in-price out-price)
  (let ((reply (ipc-call
                (format nil "(:component \"parallel\" :op \"set-model-price\" :model \"~A\" :in-price ~F :out-price ~F)"
                        (sexp-escape-lisp model)
                        (coerce in-price 'double-float)
                        (coerce out-price 'double-float)))))
    (when (ipc-reply-error-p reply)
      (error "parallel set price failed: ~A" reply))
    t))

(defun parallel-submit (prompt model)
  (let* ((reply (ipc-call
                 (format nil "(:component \"parallel\" :op \"submit\" :prompt \"~A\" :model \"~A\")"
                         (sexp-escape-lisp prompt) (sexp-escape-lisp model))))
         (id (when reply (ipc-extract-u64 reply ":id"))))
    (unless (and id (>= id 0))
      (error "parallel submit failed: ~A" (or reply "no reply")))
    id))

(defun parallel-run-pending (&optional (max-parallel 3))
  (let ((reply (ipc-call
                (format nil "(:component \"parallel\" :op \"run-pending\" :max-parallel ~D)" max-parallel))))
    (when (ipc-reply-error-p reply)
      (error "parallel run pending failed: ~A" reply))
    t))

(defun parallel-run-pending-async (&optional (max-parallel 3))
  "Run pending tasks asynchronously — results arrive via unified actor mailbox.
   Returns list of (:task-id T :actor-id A :model M) plists for Lisp-side tracking."
  (let ((reply (ipc-call
                (format nil "(:component \"parallel\" :op \"run-pending-async\" :max-parallel ~D)" max-parallel))))
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
                (format nil "(:component \"parallel\" :op \"task-result\" :task-id ~D)" task-id))))
    (or (ipc-extract-value reply)
        (error "parallel task result failed: ~A" (or reply "no reply")))))

(defun parallel-report ()
  (or (ipc-extract-value
       (ipc-call "(:component \"parallel\" :op \"report\")"))
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
  "Ticks with zero output delta before killing an actor (progress-based, not time-based).
   CLI agents (Claude Code) can work for minutes on complex tasks — 180 ticks = 3 min."
  (max 5 (or (ignore-errors (model-policy-actor-stall-threshold)) 180)))

(defun %swarm-extract-user-task (prompt)
  "Extract the raw user task from a full LLM prompt.
   The conductor assembles prompts as: DNA + bootstrap + presentation + personality + USER_TASK.
   CLI agents (Claude Code) have their own system prompt — they only need the task.

   Priority:
   1. If BASEBAND EXTERNAL DATA boundary markers exist -> extract content between them
   2. If USER_TASK: marker exists -> extract after it, then check for nested EXTERNAL DATA
   3. Fall back to prompt as-is"
  (let* ((p (or prompt ""))
         (ext-start-prefix "=== EXTERNAL DATA")
         (ext-end "=== END EXTERNAL DATA ==="))
    ;; Priority 1: BASEBAND envelope with EXTERNAL DATA boundaries
    (let ((ext-start-pos (search ext-start-prefix p)))
      (when ext-start-pos
        (let* ((after-marker (subseq p ext-start-pos))
               ;; Find the end of the start marker line (after "=== EXTERNAL DATA [xxx] ===")
               (nl-pos (position #\Newline after-marker))
               (data-start (if nl-pos (+ ext-start-pos nl-pos 1) ext-start-pos))
               (end-pos (search ext-end p :start2 data-start)))
          (when end-pos
            (return-from %swarm-extract-user-task
              (string-trim '(#\Space #\Newline #\Tab #\Return)
                           (subseq p data-start end-pos)))))))
    ;; Priority 2: USER_TASK: marker
    (let* ((marker "USER_TASK:")
           (pos (search marker p)))
      (when pos
        (let ((after-task (string-trim '(#\Space #\Newline #\Tab #\Return)
                                       (subseq p (+ pos (length marker))))))
          ;; Check for nested EXTERNAL DATA within USER_TASK content
          (let ((nested-start (search ext-start-prefix after-task)))
            (when nested-start
              (let* ((after-nested (subseq after-task nested-start))
                     (nl-pos (position #\Newline after-nested))
                     (data-start (if nl-pos (+ nested-start nl-pos 1) nested-start))
                     (end-pos (search ext-end after-task :start2 data-start)))
                (when end-pos
                  (return-from %swarm-extract-user-task
                    (string-trim '(#\Space #\Newline #\Tab #\Return)
                                 (subseq after-task data-start end-pos)))))))
          (return-from %swarm-extract-user-task after-task))))
    ;; Priority 3: no markers — prompt is already a raw task
    p))

(defun %swarm-cli-delegation-prompt (user-task)
  "Build a concise delegation prompt for a CLI subagent.
   Keep it short and direct — the CLI has its own intelligence.
   Do NOT include code examples, implementation details, or architecture guidance.
   The subagent is a capable developer that decides HOW to implement.

   Defense-in-depth: strip any remaining BASEBAND/structural artifacts."
  (let ((task (or user-task "")))
    ;; Last-resort extraction if structural artifacts leaked through
    (when (or (search "[BASEBAND CHANNEL]" task)
              (search "USER_TASK:" task)
              (search "=== EXTERNAL DATA" task))
      (let ((ext-start (search "=== EXTERNAL DATA" task)))
        (when ext-start
          (let* ((after (subseq task ext-start))
                 (nl (position #\Newline after))
                 (data-start (if nl (+ ext-start nl 1) ext-start))
                 (end-pos (search "=== END EXTERNAL DATA ===" task :start2 data-start)))
            (when end-pos
              (setf task (string-trim '(#\Space #\Newline #\Tab #\Return)
                                      (subseq task data-start end-pos))))))))
    (format nil "~A" task)))

(defun %swarm-spawn-cli-actor (model prompt &optional originating-signal orchestration-ctx
                                                       swarm-group-id)
  "Spawn a CLI subagent as a tmux actor. Returns actor-id. Does NOT block.
   Extracts user task from full LLM prompt — CLI agents don't need DNA/personality context.
   Generates a supervision spec BEFORE spawning for closed-loop verification."
  (let* ((cli (%swarm-cli-id model))
         (workdir (or (ignore-errors (config-get-for "conductor" "workdir"))
                      (namestring (user-homedir-pathname))))
         (user-task (%swarm-extract-user-task prompt))
         ;; Generate supervision spec BEFORE spawning agent
         (supervision (ignore-errors (%supervision-classify-task user-task)))
         (spec-sexp (when supervision
                      (ignore-errors
                        (%supervision-freeze-spec nil
                         (getf supervision :taxonomy)
                         (getf supervision :assertions)))))
         ;; Recall past supervision failures for similar tasks
         (past-mistakes (ignore-errors
                          (memory-recent :limit 3 :class :supervision)))
         (cli-prompt (if past-mistakes
                         (format nil "~A~%~%LEARNING FROM PAST SUPERVISION:~%~{- ~A~%~}"
                                 (%swarm-cli-delegation-prompt user-task)
                                 (mapcar (lambda (m)
                                           (%clip-prompt (or (ignore-errors
                                                               (memory-entry-content m))
                                                             (princ-to-string m))
                                                         150))
                                         past-mistakes))
                         (%swarm-cli-delegation-prompt user-task)))
         (actor-id (tmux-spawn cli workdir cli-prompt)))
    (when (%trace-level-p :standard)
      (trace-event "actor-spawned" :agent
                   :metadata (list :actor-id actor-id
                                   :model model
                                   :cli cli
                                   :cli-prompt-length (length (or cli-prompt ""))
                                   :workdir workdir
                                   :supervision-spec-p (not (null spec-sexp)))))
    ;; Update spec with actual task-id
    (when spec-sexp
      (ignore-errors (%supervision-update-task-id spec-sexp actor-id)))
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
                   :duration-ms 0
                   :swarm-group-id swarm-group-id
                   :supervision-spec spec-sexp
                   :supervision-grade nil
                   :supervision-confidence nil)))
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

(defun %swarm-subagent-brief-context (model)
  "Inject concise system context when delegating to an OpenRouter subagent.
   Template loaded from config/swarm.sexp :prompts :subagent-context."
  (let ((template (%swarm-prompt-template :subagent-context
                    "[SYSTEM CONTEXT] You are a subagent in the Harmonia swarm (model: ~A).")))
    (format nil template model)))

;;; --- DAG-based task decomposition ---

(defun %swarm-dag-software-dev-p (prompt chain)
  "Return T if this should be a DAG software-dev task with cross-audit.
   Requires both claude-code and codex available in the chain."
  (and (> (length (or prompt "")) 0)
       (some (lambda (m) (%swarm-starts-with-p m "cli:claude")) chain)
       (some (lambda (m) (%swarm-starts-with-p m "cli:codex")) chain)))

(defun %swarm-dag-split-work (prompt)
  "Split a software-dev prompt into two equal work units.
   Extracts user task from full LLM prompt first.
   Returns (values work-a work-b) where each is a focused subtask."
  (let ((p (%swarm-extract-user-task prompt)))
    ;; For now, both agents get the full task. The audit step adds the value.
    (values p p)))

(defun %swarm-dag-spawn-with-audit (prompt originating-signal orchestration-ctx group-id)
  "Spawn a DAG: both CLI agents do the task, then cross-audit each other.
   Returns group-id. Results delivered via actor mailbox."
  (when (%trace-level-p :standard)
    (trace-event "dag-spawned" :agent
                 :metadata (list :group-id group-id
                                 :pattern "cross-audit")))
  (multiple-value-bind (work-claude work-codex)
      (%swarm-dag-split-work prompt)
    (let* ((workdir (or (ignore-errors (config-get-for "conductor" "workdir"))
                        (namestring (user-homedir-pathname))))
           ;; Phase 1: Implementation (parallel) — directive from config
           (impl-suffix (%swarm-prompt-template :dag-implementer-suffix
                          "You are the primary implementer. Your work will be audited by a peer."))
           (claude-prompt (format nil "~A~%~%~A" work-claude impl-suffix))
           (codex-prompt (format nil "~A~%~%~A" work-codex impl-suffix))
           (claude-id (tmux-spawn "claude-code" workdir claude-prompt))
           (codex-id (tmux-spawn "codex" workdir codex-prompt)))
      ;; Register both as actors in the same group
      (dolist (pair (list (cons claude-id "cli:claude-code")
                          (cons codex-id "cli:codex")))
        (let ((record (make-actor-record
                       :id (car pair)
                       :model (cdr pair)
                       :prompt prompt
                       :state :running
                       :spawned-at (get-universal-time)
                       :last-heartbeat (get-universal-time)
                       :stall-ticks 0
                       :originating-signal originating-signal
                       :orchestration-context
                       (append (or orchestration-ctx '())
                               (list :dag-phase :implement
                                     :dag-peer-id (if (= (car pair) claude-id) codex-id claude-id)
                                     :dag-audit-pending t))
                       :cost-usd 0.0
                       :duration-ms 0
                       :swarm-group-id group-id)))
          (setf (gethash (car pair) (runtime-state-actor-registry *runtime*)) record)
          (push (car pair) (runtime-state-actor-pending *runtime*))))
      group-id)))

(defvar *swarm-group-counter* 0)

(defun parallel-solve (prompt &key return-structured preferred-models max-subagents
                                   originating-signal orchestration-context)
  "Spawn N subagents with different model/cost profiles, then return best + report.
   CLI models are spawned as non-blocking tmux actors. If ALL models are CLI,
   returns (values :deferred nil nil nil) — results delivered later by %tick-actor-deliver.
   For software-dev tasks with both CLIs available, uses DAG pattern with cross-audit."
  (when (%trace-level-p :minimal)
    (trace-event "parallel-solve" :agent
                 :metadata (list :max-subagents (or max-subagents 0)
                                 :preferred-models (format nil "~{~A~^,~}" (or preferred-models '()))
                                 :group-id (1+ *swarm-group-counter*)
                                 :dag-mode (%swarm-dag-software-dev-p
                                            prompt (or preferred-models '())))))
  (let* ((n (max 1 (or max-subagents (parallel-get-subagent-count))))
         (group-id (incf *swarm-group-counter*))
         (chain (or preferred-models
                    (model-escalation-chain prompt (choose-model prompt))))
         (queue (copy-list chain))
         (jobs '())
         (results '())
         (scheduled 0)
         (cli-spawned 0)
         (used-parallel nil)
         (parallel-routed nil))
    ;; DAG path: for software-dev tasks with both CLIs, use cross-audit pattern
    (when (and (>= n 2) (%swarm-dag-software-dev-p prompt chain))
      (handler-case
          (progn
            (%swarm-dag-spawn-with-audit prompt originating-signal
                                         orchestration-context group-id)
            (return-from parallel-solve
              (if return-structured
                  (values :deferred nil nil nil)
                  :deferred)))
        (error (_)
          (declare (ignore _))
          ;; Fall through to normal path on DAG spawn failure
          nil)))
    (loop while (and queue (< scheduled n)) do
      (let ((m (pop queue)))
        (if (%swarm-cli-model-p m)
            ;; Non-blocking: spawn tmux actor
            (handler-case
                (progn
                  (%swarm-spawn-cli-actor m prompt originating-signal
                                          (or orchestration-context
                                              (list :chain chain
                                                    :prepared-prompt prompt))
                                          group-id)
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
              (push (cons (parallel-submit
                           (format nil "[subagent model=~A]~%~A~%~%~A"
                                   m
                                   (%swarm-subagent-brief-context m)
                                   prompt)
                           m) m) jobs)
              (setf used-parallel t)
              (incf scheduled)))))
    ;; If ALL scheduled models were CLI, return :deferred
    (when (and (> cli-spawned 0) (null jobs) (null results))
      (return-from parallel-solve
        (if return-structured
            (values :deferred nil nil nil)
            :deferred)))
    ;; Run OpenRouter jobs asynchronously — results arrive via unified actor mailbox.
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
                            :duration-ms 0
                            :swarm-group-id group-id)))
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
