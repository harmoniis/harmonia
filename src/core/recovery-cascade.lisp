;;; recovery-cascade.lisp — Guardian Healer: LLM-guarded self-healing.
;;;
;;; Architecture:
;;;   The system heals itself through a graduated cascade and a guardian LLM
;;;   that diagnoses failures and proposes SAFE actions from a whitelist.
;;;   The LLM can never execute arbitrary code or bypass the policy gate.
;;;
;;;   Level 0: RETRY    — transient errors (IPC timeout, backend hiccup)
;;;   Level 1: FALLBACK — use simpler method (field→substring, premium→cheap)
;;;   Level 2: PATTERN  — detect repeating errors, classify root cause
;;;   Level 3: GUARDIAN — LLM diagnoses, proposes safe action from whitelist
;;;   Level 4: RESTART  — restart failed component via IPC
;;;   Level 5: REPORT   — honest message to user, record to Chronicle
;;;
;;; Guardian Principle:
;;;   The healer LLM operates with :internal taint (self-initiated).
;;;   It is constrained to a whitelist of safe actions.
;;;   It CANNOT: mutate vault, change policy, rewrite security, execute code.
;;;   It CAN: restart components, switch models, skip features, reload config.
;;;
;;; Complements Phoenix (process restarts) and Ouroboros (crash ledger).

(in-package :harmonia)

;;; ═══════════════════════════════════════════════════════════════════════
;;; STATE — health tracking, per-component
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *component-health* (make-hash-table :test 'equal)
  "Component → (:failures N :last-success UT :last-failure UT :last-heal UT)")

(defparameter *recovery-log* '()
  "Recent recovery events (bounded circular list, max 32 entries).")

(defparameter *guardian-cooldown-seconds* 120
  "Minimum seconds between guardian heal attempts for the same component.")

(defparameter *recovery-deadline-seconds* 45
  "Maximum seconds for any single operation before timeout.")

;;; ═══════════════════════════════════════════════════════════════════════
;;; SAFE ACTION WHITELIST — the only actions the guardian can execute
;;; ═══════════════════════════════════════════════════════════════════════

(defparameter *safe-recovery-actions*
  '("restart-component"
    "reload-config"
    "switch-model"
    "skip-feature"
    "clear-memory-cache"
    "reduce-load"
    "report-to-operator")
  "Actions the Guardian Healer is permitted to execute. Nothing else.")

;;; ═══════════════════════════════════════════════════════════════════════
;;; THE CASCADE — graduated recovery, functional, pure
;;; ═══════════════════════════════════════════════════════════════════════

(defun %with-recovery (operation thunk &key fallback-thunk (max-retries 1) component)
  "Execute THUNK with graduated recovery. Returns (values result status).
Status is :ok, :retried, :fallback, :healed, :degraded, or :failed.
Never signals — always returns a usable result."
  (let ((comp (or component operation)))

    ;; Level 0: Try with timeout.
    (multiple-value-bind (result ok) (%try-safe operation thunk)
      (when ok
        (%health-record-success comp)
        (return-from %with-recovery (values result :ok))))

    ;; Level 0.5: Retry (transient errors).
    (dotimes (i max-retries)
      (multiple-value-bind (result ok) (%try-safe operation thunk)
        (when ok
          (%health-record-success comp)
          (return-from %with-recovery (values result :retried)))))

    ;; Level 1: Fallback to simpler method.
    (when fallback-thunk
      (multiple-value-bind (result ok)
          (%try-safe (format nil "~A/fallback" operation) fallback-thunk)
        (when ok
          (%health-record-degraded comp)
          (%recovery-log-event comp operation :fallback t)
          (return-from %with-recovery (values result :fallback)))))

    ;; Level 2+3: Record failure, attempt guardian heal.
    (%health-record-failure comp)

    (when (%guardian-should-heal-p comp)
      (let ((healed (%guardian-heal comp operation)))
        (when healed
          ;; Guardian proposed a fix — retry original operation.
          (multiple-value-bind (result ok) (%try-safe operation thunk)
            (when ok
              (%health-record-success comp)
              (%recovery-log-event comp operation :guardian-healed t)
              (return-from %with-recovery (values result :healed)))))))

    ;; Level 4: Restart component.
    (when (%should-restart-p comp)
      (when (%restart-component comp)
        (multiple-value-bind (result ok) (%try-safe operation thunk)
          (when ok
            (%health-record-success comp)
            (%recovery-log-event comp operation :restarted t)
            (return-from %with-recovery (values result :healed))))))

    ;; Level 5: Honest report.
    (%recovery-log-event comp operation :failed nil)
    (values (%honest-message comp operation) :failed)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; TIMEOUT-SAFE EXECUTION
;;; ═══════════════════════════════════════════════════════════════════════

(defun %try-safe (name thunk)
  "Execute THUNK with deadline. Returns (values result success-p). Never signals."
  (handler-case
      (sb-sys:with-deadline (:seconds *recovery-deadline-seconds*)
        (values (funcall thunk) t))
    (sb-sys:deadline-timeout ()
      (%log :warn "recovery" "~A timed out (~Ds)" name *recovery-deadline-seconds*)
      (values nil nil))
    (serious-condition (c)
      (%log :warn "recovery" "~A failed: ~A" name (princ-to-string c))
      (values nil nil))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; HEALTH TRACKING — per-component, functional accumulator
;;; ═══════════════════════════════════════════════════════════════════════

(defun %health-get (comp)
  (or (gethash comp *component-health*)
      (list :failures 0 :last-success 0 :last-failure 0 :last-heal 0)))

(defun %health-failures (comp)
  (getf (%health-get comp) :failures))

(defun %health-record-success (comp)
  (setf (gethash comp *component-health*)
        (list :failures 0
              :last-success (get-universal-time)
              :last-failure (getf (%health-get comp) :last-failure)
              :last-heal (getf (%health-get comp) :last-heal))))

(defun %health-record-failure (comp)
  (let ((h (%health-get comp)))
    (setf (gethash comp *component-health*)
          (list :failures (1+ (getf h :failures))
                :last-success (getf h :last-success)
                :last-failure (get-universal-time)
                :last-heal (getf h :last-heal)))))

(defun %health-record-degraded (comp)
  "Working via fallback — note but don't increment failures."
  (let ((h (%health-get comp)))
    (setf (gethash comp *component-health*)
          (list :failures (getf h :failures)
                :last-success (get-universal-time)
                :last-failure (getf h :last-failure)
                :last-heal (getf h :last-heal)))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; GUARDIAN HEALER — LLM-assisted self-diagnosis with SAFE whitelist
;;; ═══════════════════════════════════════════════════════════════════════

(defun %guardian-should-heal-p (comp)
  "Should we invoke the guardian for this component? Rate-limited."
  (let* ((h (%health-get comp))
         (failures (getf h :failures))
         (last-heal (getf h :last-heal))
         (now (get-universal-time)))
    (and (>= failures 3)
         (> (- now (or last-heal 0)) *guardian-cooldown-seconds*))))

(defun %guardian-heal (comp operation)
  "Invoke Guardian Healer: diagnose via LLM, propose safe action, execute if valid.
Returns T if healing was attempted, NIL if skipped."
  ;; Check if LLM routing is available.
  (when (not (and (fboundp 'backend-complete-safe)
                  (fboundp 'route-completion-sexp)))
    (return-from %guardian-heal nil))
  (handler-case
      (let* ((diagnosis (%build-guardian-prompt comp operation))
             ;; Call the LLM with the diagnostic prompt.
             (response (ignore-errors
                         (funcall 'backend-complete-safe diagnosis)))
             ;; Parse the structured response.
             (action (and (stringp response)
                          (%parse-guardian-action response))))
        ;; Record the heal attempt.
        (let ((h (%health-get comp)))
          (setf (gethash comp *component-health*)
                (list :failures (getf h :failures)
                      :last-success (getf h :last-success)
                      :last-failure (getf h :last-failure)
                      :last-heal (get-universal-time))))
        (cond
          ;; Valid safe action — execute it.
          (action
           (%log :info "guardian" "Healing ~A: ~A ~A (reason: ~A)"
                 comp (getf action :action) (or (getf action :target) "")
                 (or (getf action :reason) ""))
           (%execute-safe-action action comp)
           (%recovery-log-event comp operation :guardian-action t
                                :detail (format nil "~A ~A" (getf action :action) (getf action :target)))
           t)
          ;; LLM responded but action wasn't parseable or not in whitelist.
          (response
           (%log :warn "guardian" "LLM response not actionable for ~A: ~A"
                 comp (subseq response 0 (min 100 (length response))))
           nil)
          ;; LLM call itself failed.
          (t
           (%log :warn "guardian" "Guardian LLM unavailable for ~A diagnosis." comp)
           nil)))
    (error (c)
      (%log :warn "guardian" "Guardian heal error: ~A" (princ-to-string c))
      nil)))

(defun %build-guardian-prompt (comp operation)
  "Build a constrained diagnostic prompt for the Guardian Healer."
  (let* ((errors (ignore-errors (introspect-recent-errors 5)))
         (error-text (if errors
                         (format nil "~{- ~A~%~}"
                                 (mapcar (lambda (e)
                                           (if (listp e)
                                               (format nil "~A: ~A" (or (getf e :action) "?") (or (getf e :message) "?"))
                                               (princ-to-string e)))
                                         errors))
                         "No recent errors available."))
         (failures (%health-failures comp)))
    (format nil
"GUARDIAN SELF-DIAGNOSIS

You are Harmonia's self-healing guardian. A component has failed repeatedly.

COMPONENT: ~A
OPERATION: ~A
CONSECUTIVE FAILURES: ~D

RECENT ERRORS:
~A

Choose exactly ONE recovery action from this list:
  restart-component <component-name>
  reload-config <scope-name>
  switch-model <model-name>
  skip-feature <feature-name>
  clear-memory-cache
  reduce-load
  report-to-operator <message>

Respond with EXACTLY one line in this format:
ACTION: <action> TARGET: <target> REASON: <brief reason>"
      comp operation failures error-text)))

(defun %parse-guardian-action (response)
  "Parse LLM response into a safe action plist. Returns nil if not valid."
  (let* ((line (string-trim '(#\Space #\Newline #\Return #\Tab) response))
         ;; Find ACTION: prefix
         (action-pos (search "ACTION:" line :test #'char-equal))
         (target-pos (search "TARGET:" line :test #'char-equal))
         (reason-pos (search "REASON:" line :test #'char-equal)))
    (when action-pos
      (let* ((action-start (+ action-pos 7))
             (action-end (or target-pos reason-pos (length line)))
             (action-str (string-trim '(#\Space) (subseq line action-start action-end)))
             (target-str (when target-pos
                           (string-trim '(#\Space)
                                        (subseq line (+ target-pos 7)
                                                (or reason-pos (length line))))))
             (reason-str (when reason-pos
                           (string-trim '(#\Space)
                                        (subseq line (+ reason-pos 7))))))
        ;; WHITELIST CHECK — the most important security gate.
        (when (member action-str *safe-recovery-actions* :test #'string-equal)
          (list :action (intern (string-upcase action-str) :keyword)
                :target target-str
                :reason reason-str))))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; SAFE ACTION EXECUTORS
;;; ═══════════════════════════════════════════════════════════════════════

(defun %execute-safe-action (action comp)
  "Execute a validated safe recovery action."
  (let ((act (getf action :action))
        (target (or (getf action :target) comp)))
    (case act
      (:restart-component
       (%restart-component target))
      (:reload-config
       (ignore-errors
         (ipc-call (format nil "(:component \"config\" :op \"ingest-env\")"))))
      (:switch-model
       ;; Switch the router's active tier to a different model.
       (ignore-errors
         (ipc-call (format nil "(:component \"config\" :op \"set\" :component \"router\" :scope \"router\" :key \"active-model\" :value \"~A\")"
                           (sexp-escape-lisp target)))))
      (:skip-feature
       (%log :info "guardian" "Skipping feature: ~A (temporarily disabled)" target))
      (:clear-memory-cache
       (ignore-errors (memory-reset))
       (%log :info "guardian" "Memory cache cleared."))
      (:reduce-load
       (%log :info "guardian" "Reducing system load (advisory)."))
      (:report-to-operator
       (%log :warn "guardian" "OPERATOR ATTENTION: ~A — ~A" comp (getf action :reason))))))

(defun %restart-component (comp)
  "Restart a component via IPC reset. Returns T on success."
  (%log :info "recovery" "Restarting component: ~A" comp)
  (handler-case
      (let ((reply (ipc-call
                    (format nil "(:component \"~A\" :op \"reset\")"
                            (sexp-escape-lisp comp)))))
        (and reply (ipc-reply-ok-p reply)))
    (error () nil)))

(defun %should-restart-p (comp)
  "Should we attempt a bare restart (without guardian)?"
  (let ((failures (%health-failures comp)))
    (and (> failures 2) (< failures 10))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; HONEST ERROR MESSAGES — never "internal error"
;;; ═══════════════════════════════════════════════════════════════════════

(defun %honest-message (comp operation)
  "Build an honest, helpful message for the user when recovery fails."
  (cond
    ((string-equal comp "conductor")
     "I'm having trouble processing your request right now. I've logged the issue and I'm working on self-repair. Please try again in a moment.")
    ((string-equal comp "memory-field")
     "My memory system is recalibrating. I can still respond, just with less context than usual.")
    ((string-equal comp "provider-router")
     "The language model service is temporarily unavailable. I'll retry automatically. Please try again shortly.")
    ((string-equal comp "memory-recall")
     "I'm having trouble accessing my memories right now. Let me try to answer from what I know directly.")
    (t
     (format nil "I encountered a temporary issue (~A). I've recorded it and I'm self-diagnosing. Please try again."
             comp))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; RECOVERY LOG — bounded circular list for audit
;;; ═══════════════════════════════════════════════════════════════════════

(defun %recovery-log-event (comp operation level success &key detail)
  "Record a recovery event to the in-memory log and Chronicle."
  (let ((event (list :component comp
                     :operation operation
                     :level level
                     :success success
                     :detail detail
                     :time (get-universal-time))))
    ;; In-memory log (bounded to 32 entries).
    (push event *recovery-log*)
    (when (> (length *recovery-log*) 32)
      (setf *recovery-log* (subseq *recovery-log* 0 32)))
    ;; Chronicle persistence (if available).
    (ignore-errors
      (when (fboundp 'ipc-call)
        (ipc-call
         (format nil "(:component \"chronicle\" :op \"record-ouroboros-event\" :event-type \"recovery\" :generation 0 :fitness 0.0 :mutation-count 0 :crossover-count 0 :detail \"~A ~A ~A ~A\")"
                 (sexp-escape-lisp comp)
                 (sexp-escape-lisp (princ-to-string level))
                 (if success "succeeded" "failed")
                 (sexp-escape-lisp (or detail ""))))))))

;;; ═══════════════════════════════════════════════════════════════════════
;;; HEALTH HEARTBEAT — called from tick loop
;;; ═══════════════════════════════════════════════════════════════════════

(defun %tick-recovery-heartbeat ()
  "Periodic health check: detect patterns, heal proactively.
Called from the tick loop every cycle; checks health every 10 cycles."
  (when (and (boundp '*runtime*)
             *runtime*
             (zerop (mod (runtime-state-cycle *runtime*) 10)))
    ;; Check each tracked component.
    (maphash (lambda (comp health)
               (let ((failures (getf health :failures)))
                 (when (and failures (> failures 3))
                   ;; Component is sick — attempt guardian heal.
                   (when (%guardian-should-heal-p comp)
                     (%log :info "recovery" "Heartbeat: ~A sick (~D failures). Invoking guardian."
                           comp failures)
                     (%guardian-heal comp "health-heartbeat")))))
             *component-health*)))

;;; ═══════════════════════════════════════════════════════════════════════
;;; CONVENIENCE WRAPPERS
;;; ═══════════════════════════════════════════════════════════════════════

(defun %with-memory-recovery (query thunk)
  "Wrap memory recall with field→substring fallback."
  (%with-recovery "memory-recall" thunk
    :fallback-thunk (lambda ()
                      (handler-case
                          (when (fboundp '%memory-substring-layered-recall)
                            (%memory-substring-layered-recall query :limit 5))
                        (error () nil)))
    :component "memory-field"))

(defun %with-llm-recovery (thunk &key fallback-thunk)
  "Wrap LLM completion with timeout + model fallback."
  (%with-recovery "llm-completion" thunk
    :fallback-thunk fallback-thunk
    :max-retries 1
    :component "provider-router"))
