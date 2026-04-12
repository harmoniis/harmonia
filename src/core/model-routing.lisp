;;; model-routing.lisp — Routing tier management (/auto /eco /premium /free).

(in-package :harmonia)

;;; --- Routing Tier System ---

(defparameter *routing-tier* :auto
  "Current routing tier: :auto :eco :premium :free.
   Set via /auto /eco /premium /free TUI commands.")

(defparameter *last-task-kind* :general
  "Last task kind classified by %task-kind — used for route feedback.")

(defparameter *routing-rules-sexp*
  '(:version 1
    :task-tier-hints
      ((:task :memory-ops :preferred-tier :eco)
       (:task :critical-reasoning :preferred-tier :premium))
    :model-bans ()
    :model-boosts ()
    :cascade-config (:max-escalations 3 :confidence-threshold 0.7))
  "Self-rewriting routing rules. Signalograd mutates at runtime.")

(defun %load-routing-tier ()
  "Load routing tier from config-store (persists across sessions)."
  (let ((raw (and (fboundp 'config-get-for)
                  (config-get-for "router" "active-tier"))))
    (setf *routing-tier*
          (cond
            ((and raw (string= raw "eco")) :eco)
            ((and raw (string= raw "premium")) :premium)
            ((and raw (string= raw "free")) :free)
            (t :auto)))))

;;; ===============================================================================
;;; MODEL POOL -- declarative, functional, tier-aware
;;;
;;; /free    -> ONLY free models (cost=0). Nothing else.
;;; /eco     -> free + cheap. NEVER premium. Best intelligence for least cost.
;;; /auto    -> all models. Encoder complexity decides: simple->cheap, complex->premium.
;;;            Best intelligence x speed for the price.
;;; /premium -> ONLY premium models. Best intelligence regardless of cost.
;;; ===============================================================================

(defun %tier-model-pool (tier)
  "Return model IDs eligible for TIER. Pure functional -- profile attributes decide.
   Demoted models and banned models are excluded from all pools."
  (let ((ids '())
        (bans (or (getf *routing-rules-sexp* :model-bans) '())))
    (dolist (p *model-profiles* (nreverse ids))
      (let ((cost (getf p :cost 10))
            (quality (getf p :quality 1))
            (ptier (getf p :tier))
            (mid (getf p :id)))
        ;; Skip demoted models and banned models from ALL pools.
        (unless (or (eq ptier :demoted)
                    (member mid bans :test #'string=))
          (when (case tier
                  ;; FREE: only cost=0 models. Self-hosted + free-tier.
                  (:free (= cost 0))
                  ;; ECO: free + cheap. Never premium/frontier.
                  (:eco (and (not (member ptier '(:pro :frontier) :test #'eq))
                             (<= cost 5)))
                  ;; PREMIUM: only high-quality models. Ignore cost.
                  (:premium (or (member ptier '(:frontier :pro :fast-smart :thinking) :test #'eq)
                                (>= quality 7)))
                  ;; AUTO: everything -- scoring decides.
                  (:auto t))
            (push mid ids)))))))

(defun %tier-weight-bias (tier)
  "Scoring bias per tier. Shifts what matters for ranking within the pool."
  (case tier
    ;; Free: maximize speed (all are free, so price irrelevant).
    (:free    '(:speed 0.20 :reasoning 0.10))
    ;; Eco: maximize intelligence per dollar. Speed secondary.
    (:eco     '(:price 0.20 :reasoning 0.10 :speed 0.05))
    ;; Premium: maximize intelligence. Ignore price entirely.
    (:premium '(:reasoning 0.15 :completion 0.10 :price -0.20))
    ;; Auto: balanced -- the default weights are already optimal.
    (:auto    '())))

(defun %auto-tier-pool-from-routing-context (routing-ctx)
  "Auto mode: all models in pool. The scoring function decides everything.
   No imperative routing -- encoder complexity feeds into the score, not the pool filter."
  (declare (ignore routing-ctx))
  (%tier-model-pool :auto))

(defun %seed-score-with-bias (profile bias)
  "Score a model. Tier bias shifts weights. REPL fluency demotes bad models.
   Signalograd modulates all weights. The math decides, not if/else."
  (let* ((base-weights (copy-tree (or (getf *model-evolution-policy* :seed-weights) '())))
         ;; Apply tier bias
         (weights (progn
                    (loop for (k v) on bias by #'cddr
                          do (setf (getf base-weights k)
                                   (+ (or (getf base-weights k) 0.0) v)))
                    base-weights))
         (model-id (getf profile :id))
         (entry (%score-entry-for-model model-id))
         ;; Signalograd-modulated weights
         (w-price (signalograd-routing-weight :price (or (getf weights :price) 0.35) *runtime*))
         (w-speed (signalograd-routing-weight :speed (or (getf weights :speed) 0.20) *runtime*))
         (w-success (signalograd-routing-weight :success (or (getf weights :success) 0.20) *runtime*))
         (w-reasoning (signalograd-routing-weight :reasoning (or (getf weights :reasoning) 0.15) *runtime*))
         (w-vitruvian (or (getf weights :vitruvian) 0.10))
         (weight-sum (max 0.001 (+ w-price w-speed w-success w-reasoning w-vitruvian)))
         ;; Raw signals
         (price (/ (%usd-price-score profile) 10.0))
         (speed (%observed-latency-score entry profile))
         (success (or (and entry (getf entry :success-rate)) 0.5))
         (reasoning (%reasoning-score profile))
         (vitruvian (or (and entry (getf entry :vitruvian-signal))
                        (%runtime-vitruvian-signal)))
         ;; REPL fluency: models that can't use REPL get demoted.
         ;; This is the adaptive mechanism -- bad models sink, good ones rise.
         (repl-fluency (if (and (boundp '*repl-model-perf*)
                                (fboundp '%repl-fluency))
                           (funcall '%repl-fluency model-id)
                           0.5))
         ;; Base score from weighted signals
         (base-score (+ (* (/ w-price weight-sum) price)
                        (* (/ w-speed weight-sum) speed)
                        (* (/ w-success weight-sum) success)
                        (* (/ w-reasoning weight-sum) reasoning)
                        (* (/ w-vitruvian weight-sum) vitruvian))))
    ;; Final score: base x REPL fluency. Bad REPL speakers get multiplied down.
    (* base-score (max 0.1 repl-fluency))))

(defun %score-and-rank-within-tier (model-ids task)
  "Re-rank MODEL-IDS using tier-biased weights + signalograd."
  (declare (ignore task))
  (let* ((bias (%tier-weight-bias *routing-tier*))
         (scored
           (mapcar (lambda (id)
                     (let ((p (%profile-by-id id)))
                       (if p
                           (cons id (%seed-score-with-bias p bias))
                           (cons id 0.0))))
                   model-ids)))
    (mapcar #'car (sort scored #'> :key #'cdr))))

(defun %selection-chain-tiered (prompt &optional routing-ctx)
  "ONE function. Tier sets the pool. Scoring ranks within the pool.
   The math decides everything: signalograd weights, REPL fluency, encoder complexity.
   No imperative routing. Pure functional: tier -> pool -> score -> ranked list."
  (declare (ignore routing-ctx))
  (%load-routing-tier)
  (let* ((pool (%tier-model-pool *routing-tier*))
         (task (%task-kind prompt))
         (scored (when pool (%score-and-rank-within-tier pool task))))
    (or scored (list "cli:claude-code"))))

;;; --- Task Routing ---

(defun %task-routing (task-kind)
  "Return the task-routing entry for TASK-KIND from loaded policy."
  (getf *model-task-routing* task-kind))

;;; --- CLI Detection ---

(defun %detect-available-cli (cli-list)
  "Return the first CLI name from CLI-LIST that exists on PATH, or NIL."
  (dolist (cli cli-list)
    (let ((cmd (cond
                 ((string= cli "claude-code") "claude")
                 (t cli))))
      (handler-case
          (let ((output (with-output-to-string (s)
                          (sb-ext:run-program "/usr/bin/which" (list cmd)
                                              :output s :error nil :search t))))
            (when (and output (> (length (string-trim '(#\Space #\Newline) output)) 0))
              (return cli)))
        (error () nil)))))

;;; --- Software-Dev Model Selection ---

(defun %software-dev-choose (prompt)
  "Choose model for software-dev tasks: prefer local CLI, then ranked models."
  (let ((routing (%task-routing :software-dev)))
    ;; Try CLI tools first
    (let* ((cli-prefs (or (getf routing :cli-preference) '("claude-code" "codex")))
           (available-cli (%detect-available-cli cli-prefs)))
      (when available-cli
        (return-from %software-dev-choose
          (format nil "cli:~A" available-cli))))
    ;; Fall back to ranked models
    (let ((cands (%model-candidates :software-dev :limit 3)))
      (or (first cands)
          (getf (first *model-profiles*) :id)))))

;;; --- Memory-Ops Model Selection ---

(defun %memory-ops-choose ()
  "Choose cheapest model tagged for memory-ops."
  (let ((routing (%task-routing :memory-ops)))
    (if routing
        (let ((models (getf routing :models)))
          (or (first models) "google/gemini-3.1-flash-lite-preview"))
        ;; fallback: cheapest profile
        (getf (first (%profiles-by-cost)) :id))))

;;; --- choose-model (refactored into three paths) ---

(defun %choose-model-default (prompt)
  "Default model selection: heuristic or planner."
  (let* ((task (%task-kind prompt))
         (cands (%model-candidates task :limit 6))
         (fallback (or (first cands)
                       (getf (first *model-profiles*) :id))))
    (if (not (%planner-enabled-p))
        fallback
        (handler-case
            (let* ((decision-prompt
                     (format nil
                             "Pick exactly one model id from this list for task kind ~A. Reply ONLY with model id.~%Candidates: ~{~A~^, ~}~%Prompt: ~A"
                             task cands prompt))
                   (decision-with-dna (dna-compose-llm-prompt decision-prompt :mode :planner))
                   (picked (string-trim '(#\Space #\Newline #\Tab)
                                        (backend-complete decision-with-dna (%planner-model)))))
              (if (%profile-by-id picked) picked fallback))
          (error (_)
            (declare (ignore _))
            fallback)))))

(defun choose-model (prompt &optional routing-ctx)
  "Select the best model for PROMPT, respecting the active routing tier.
   ROUTING-CTX is the optional :routing plist from the signal envelope."
  (setf *last-task-kind* (%task-kind prompt))
  (let ((chain (%selection-chain-tiered prompt routing-ctx)))
    (or (first chain)
        (getf (first *model-profiles*) :id))))

;;; --- Escalation Chain ---

(defun model-escalation-chain (prompt chosen)
  (let ((chain (%selection-chain prompt)))
    (if (member chosen chain :test #'string=)
        (or (member chosen chain :test #'string=) chain)
        chain)))
