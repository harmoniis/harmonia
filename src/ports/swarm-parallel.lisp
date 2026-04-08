;;; swarm-parallel.lisp — Parallel orchestration: task extraction, CLI delegation, DAG, parallel-solve.

(in-package :harmonia)

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
  (max 5 (or (handler-case (model-policy-actor-stall-threshold) (error () nil)) 180)))

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
         (workdir (or (handler-case (config-get-for "conductor" "workdir") (error () nil))
                      (namestring (user-homedir-pathname))))
         (user-task (%swarm-extract-user-task prompt))
         ;; Generate supervision spec BEFORE spawning agent
         (supervision (handler-case (%supervision-classify-task user-task) (error () nil)))
         (spec-sexp (when supervision
                      (handler-case

                          (%supervision-freeze-spec nil
                         (getf supervision :taxonomy)
                         (getf supervision :assertions)

                        (error () nil)))))
         ;; Recall past supervision failures for similar tasks
         (past-mistakes (handler-case
     (memory-recent :limit 3 :class :supervision)
   (error () nil)))
         (cli-prompt (if past-mistakes
                         (format nil "~A~%~%LEARNING FROM PAST SUPERVISION:~%~{- ~A~%~}"
                                 (%swarm-cli-delegation-prompt user-task)
                                 (mapcar (lambda (m)
                                           (%clip-prompt (or (handler-case
     (memory-entry-content m)
   (error () nil))
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
      (handler-case (%supervision-update-task-id spec-sexp actor-id) (error () nil)))
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
    (let* ((workdir (or (handler-case (config-get-for "conductor" "workdir") (error () nil))
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
                  (handler-case (model-policy-mark-cli-cooloff (%swarm-cli-id m)) (error () nil))
                  (when (model-policy-cli-quota-exceeded-p msg)
                    (handler-case (model-policy-mark-cli-cooloff (%swarm-cli-id m)) (error () nil)))
                  (handler-case

                      (model-policy-record-outcome
                     :model m
                     :success nil
                     :latency-ms 0
                     :harmony-score 0.0
                     :cost-usd 0.0)

                    (error () nil)))))
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
          (handler-case

              (model-policy-record-outcome
             :model model
             :success success
             :latency-ms latency
             :harmony-score score
             :cost-usd cost)

            (error () nil))
          (when success
            (push entry usable-results))))
      (setf usable-results (nreverse usable-results))
      (unless usable-results
        (error "parallel solve failed: all model attempts failed"))
      (let* ((best-entry (car (sort (copy-list usable-results) #'> :key (lambda (e) (getf e :score)))))
             (best (getf best-entry :text))
             (rep (if used-parallel
                      (or (handler-case (parallel-report) (error () nil)) "parallel-report-unavailable")
                      "direct-cli")))
        (when used-parallel
          (harmonic-matrix-observe-route "orchestrator" "parallel-agents" t 1)
          (harmonic-matrix-observe-route "parallel-agents" "memory" t 1))
        (if return-structured
            (values best rep best-entry usable-results)
            (format nil "PARALLEL_BEST=~A~%PARALLEL_REPORT=~A" best rep))))))
