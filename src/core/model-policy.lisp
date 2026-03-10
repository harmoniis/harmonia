;;; model-policy.lisp — Harmonic model selection policy.

(in-package :harmonia)

(defparameter *model-policy-config-path*
  (merge-pathnames "../../config/model-policy.sexp" *boot-file*))
(defparameter *model-policy-state-path* nil)

(defun %model-policy-resolve-state-path ()
  "Resolve state path lazily (config-store not available at load time)."
  (or *model-policy-state-path*
      (setf *model-policy-state-path*
            (or (and (fboundp 'config-get-for)
                     (config-get-for "model-policy" "path"))
                (let ((root (or (and (fboundp 'config-get-for)
                                     (config-get-for "model-policy" "state-root" "global"))
                                (let ((base (or (sb-ext:posix-getenv "TMPDIR")
                                                (namestring (user-homedir-pathname)))))
                                  (concatenate 'string (string-right-trim "/" base) "/harmonia")))))
                  (concatenate 'string root "/model-policy.sexp"))))))

(defparameter *model-profiles* '())
(defparameter *model-harmony-weights* '())
(defparameter *model-task-routing* '())
(defparameter *default-model-evolution-policy*
  '(:seed-models ()
    :seed-provider-models ()
    :active-provider "openrouter"
    :seed-weights (:price 0.35 :speed 0.20 :success 0.20 :reasoning 0.15 :vitruvian 0.10)
    :seed-min-samples 3
    :last-resort-models ("google/gemini-2.5-pro"
                         "openai/gpt-5"
                         "anthropic/claude-sonnet-4.6")
    :rewrite-capable-models ("anthropic/claude-opus-4.6"
                             "openai/gpt-5"
                             "anthropic/claude-sonnet-4.6")
    :cli-preference ("claude-code" "codex")
    :cli-task-kinds (:software-dev :coding :critical-reasoning)
    :actor-stall-threshold 50
    :cli-cooloff-seconds 3600
    :cli-quota-patterns ("quota" "rate limit" "cooldown" "usage cap" "too many requests")
    :vitruvian-signal-min 0.62
    :context-summarizer-model "qwen/qwen3.5-flash-02-23"
    :context-summarizer-threshold-chars 12000
    :orchestrator-delegate-swarm t))
(defparameter *model-evolution-policy* (copy-tree *default-model-evolution-policy*))
(defparameter *cli-cooloff-until* (make-hash-table :test 'equal))

(declaim (ftype function backend-complete model-policy-get))

(defun %plist-merge (base override)
  (let ((result (copy-list (or base '()))))
    (loop for (k v) on (or override '()) by #'cddr do
      (setf (getf result k) v))
    result))

(defun %stable-unique-strings (items)
  (let ((out '()))
    (dolist (item items (nreverse out))
      (when (and item (not (member item out :test #'string=)))
        (push item out)))))

(defun %normalize-evolution-policy (policy)
  (setf (getf policy :seed-models)
        (%stable-unique-strings (or (getf policy :seed-models) '())))
  policy)

(defun %model-policy-read-file (path)
  (with-open-file (in path :direction :input)
    (let ((*read-eval* nil))
      (read in nil nil))))

(defun %model-policy-load-source ()
  (let ((state-path (%model-policy-resolve-state-path)))
    (cond
      ((probe-file state-path)
       (%model-policy-read-file state-path))
      ((probe-file *model-policy-config-path*)
       (%model-policy-read-file *model-policy-config-path*))
      (t
       (error "model policy config missing: ~A" *model-policy-config-path*)))))

(defun model-policy-load ()
  (let ((src (%model-policy-load-source)))
    (setf *model-profiles* (copy-tree (getf src :profiles)))
    (setf *model-harmony-weights* (copy-tree (getf src :weights)))
    (setf *model-task-routing* (copy-tree (getf src :task-routing)))
    (setf *model-evolution-policy*
          (%normalize-evolution-policy
           (%plist-merge *default-model-evolution-policy*
                         (copy-tree (getf src :evolution)))))
    (model-policy-get)))

(defun %profiles-by-cost ()
  (sort (copy-list *model-profiles*) #'< :key (lambda (p) (getf p :cost 9999))))

(defun %planner-profile-id ()
  (or (getf (find-if (lambda (p) (member :planner (getf p :tags))) *model-profiles*) :id)
      (getf (first *model-profiles*) :id)))

(defun model-policy-save ()
  (let ((path (%model-policy-resolve-state-path)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 (model-policy-get) out)
        (terpri out)))
    path))

;;; --- Task Classification ---

(defun %truth-seeking-prompt-p (prompt)
  (let ((p (string-downcase (or prompt ""))))
    (or (search "truth" p)
        (search "reality" p)
        (search "accurate" p)
        (search "accuracy" p)
        (search "fact-check" p)
        (search "fact check" p)
        (search "verify" p)
        (search "verification" p)
        (search "debunk" p)
        (search "controvers" p)
        (search "what actually" p)
        (search "what is really" p)
        (search "real-time" p)
        (search "realtime" p)
        (search "current event" p)
        (search "harmonic truth" p))))

(defun %task-kind (prompt)
  (let ((p (string-downcase prompt)))
    (cond
      ((or (search "implement" p) (search "refactor" p) (search "write code" p)
           (search "fix bug" p) (search "pull request" p) (search "pr " p)
           (search "commit" p) (search "deploy" p) (search "test suite" p)
           (search "debug" p) (search "build" p) (search "compile error" p))
       :software-dev)
      ((or (search "summarize" p) (search "compress" p) (search "memory" p)
           (search "digest" p) (search "consolidate" p) (search "journal" p))
       :memory-ops)
      ((%truth-seeking-prompt-p p) :truth-seeking)
      ((or (search "codemode" p) (search "mcp" p) (search "pipeline" p)
           (search "batch tools" p) (search "tool chain" p))
       :codemode)
      ((or (search "ocr" p) (search "image" p) (search "vision" p)) :vision)
      ((or (search "rewrite" p) (search "evolution" p) (search "architecture" p)) :critical-reasoning)
      ((or (search "plan" p) (search "orchestrate" p) (search "decision" p)) :planning)
      ((or (search "tool op=" p) (search "send" p) (search "search " p)) :tooling)
      ((or (search "code" p) (search "bug" p)) :coding)
      (t :general))))

;;; --- Task Weights ---

(defun %task-weights (task)
  (case task
    (:software-dev '(:completion 0.30 :correctness 0.25 :speed 0.08 :price 0.12
                     :token-efficiency 0.07 :orchestration-efficiency 0.12 :experience 0.06))
    (:memory-ops '(:completion 0.20 :correctness 0.10 :speed 0.25 :price 0.30
                   :token-efficiency 0.10 :orchestration-efficiency 0.05 :experience 0.00))
    (:truth-seeking '(:completion 0.28 :correctness 0.32 :speed 0.06 :price 0.06
                      :token-efficiency 0.04 :orchestration-efficiency 0.08 :experience 0.16))
    (:tooling '(:completion 0.24 :correctness 0.20 :speed 0.22 :price 0.16
                :token-efficiency 0.10 :orchestration-efficiency 0.08))
    (:vision '(:completion 0.36 :correctness 0.28 :speed 0.14 :price 0.10
               :token-efficiency 0.07 :orchestration-efficiency 0.05))
    (:critical-reasoning '(:completion 0.44 :correctness 0.25 :speed 0.08 :price 0.08
                           :token-efficiency 0.07 :orchestration-efficiency 0.08))
    (:planning '(:completion 0.34 :correctness 0.21 :speed 0.12 :price 0.10
                 :token-efficiency 0.10 :orchestration-efficiency 0.13))
    (:coding '(:completion 0.34 :correctness 0.24 :speed 0.12 :price 0.10
               :token-efficiency 0.10 :orchestration-efficiency 0.10))
    (:codemode '(:completion 0.22 :correctness 0.16 :speed 0.16 :price 0.16
                 :token-efficiency 0.16 :orchestration-efficiency 0.14))
    (t *model-harmony-weights*)))

;;; --- Task Tag Needs ---

(defun %task-need (task)
  (case task
    (:software-dev '(:software-dev :codemode :reasoning))
    (:memory-ops '(:memory-ops :cheap :fast))
    (:truth-seeking '(:truth-seeking :reasoning :web-search :x-search))
    (:vision '(:vision :ocr))
    (:critical-reasoning '(:thinking :frontier :reasoning))
    (:planning '(:planner :thinking :reasoning))
    (:tooling '(:fast :cheap))
    (:codemode '(:codemode :token-efficient :tool-use))
    (:coding '(:coding :reasoning))
    (t '(:balanced :reasoning))))

;;; --- Scoring Helpers ---

(defun %model-weight (weights k)
  (or (getf weights k) 0.0))

(defun %tag-hit-score (tags tag-a tag-b)
  (cond
    ((member tag-a tags) 1.0)
    ((member tag-b tags) 0.6)
    (t 0.2)))

;;; --- Experience Score ---

(defun %swarm-scores-path ()
  (let ((root (or (config-get-for "model-policy" "state-root" "global")
                  (let ((base (or (sb-ext:posix-getenv "TMPDIR")
                                  (namestring (user-homedir-pathname)))))
                    (concatenate 'string (string-right-trim "/" base) "/harmonia")))))
    (concatenate 'string root "/swarm_model_scores.sexp")))

(defun %load-swarm-scores ()
  (let ((path (%swarm-scores-path)))
    (if (probe-file path)
        (with-open-file (in path :direction :input)
          (let ((*read-eval* nil))
            (read in nil '())))
        '())))

(defun %experience-score (model-id)
  "Return experience score for MODEL-ID: success-rate * harmony-avg if >= 3 samples, else 0.0."
  (let* ((scores (%load-swarm-scores))
         (entry (find model-id scores :key (lambda (e) (getf e :model-id)) :test #'string=)))
    (if (and entry (>= (or (getf entry :samples) 0) 3))
        (* (or (getf entry :success-rate) 0.0)
           (or (getf entry :harmony-avg) 0.0))
        0.0)))

;;; --- Model Features (Grok etc.) ---

(defun %model-features (model-id)
  "Return the :features plist for MODEL-ID, or NIL."
  (let ((profile (%profile-by-id model-id)))
    (when profile (getf profile :features))))

(defun model-feature-params (model-id)
  "Return a readable sexp of feature flags for MODEL-ID."
  (let ((feats (%model-features model-id)))
    (if feats
        (format nil "~S" feats)
        "()")))

;;; --- USD Price Scoring ---

(defun %usd-price-score (profile)
  "Score 0-10 based on USD pricing. Cheaper = higher score."
  (let ((usd-in (getf profile :usd-in-1k))
        (usd-out (getf profile :usd-out-1k)))
    (if (and usd-in usd-out)
        (let ((avg (/ (+ usd-in usd-out) 2.0)))
          (cond
            ((<= avg 0.0) 10.0)       ; free
            ((<= avg 0.001) 8.0)      ; very cheap
            ((<= avg 0.005) 6.0)      ; moderate
            ((<= avg 0.02) 4.0)       ; expensive
            (t 2.0)))                  ; very expensive
        ;; fallback: use cost heuristic
        (/ 10.0 (max 1 (getf profile :cost 10))))))

;;; --- Model Scoring ---

(defun %model-score (profile task)
  (let* ((weights (%task-weights task))
         (tags (getf profile :tags))
         (need (%task-need task))
         (completion (getf profile :completion 1))
         (quality (getf profile :quality 1))
         (latency (getf profile :latency 10))
         (fit (loop for n in need count (member n tags)))
         (completion-s (* (%model-weight weights :completion) completion))
         (quality-s (* (%model-weight weights :correctness) quality))
         (speed-s (* (%model-weight weights :speed) (/ 10.0 (max 1 latency))))
         (price-s (* (%model-weight weights :price) (%usd-price-score profile)))
         (token-s (* (%model-weight weights :token-efficiency)
                     (max 0.0 (min 1.0 (/ 10.0 (+ (getf profile :cost 10) (* 0.5 latency)))))))
         (orch-s (* (%model-weight weights :orchestration-efficiency)
                    (%tag-hit-score tags :codemode :tool-use)))
         (exp-weight (%model-weight weights :experience))
         (exp-s (* exp-weight (%experience-score (getf profile :id)))))
    (+ completion-s quality-s speed-s price-s token-s orch-s exp-s (* 0.5 fit))))

(defun %profile-by-id (id)
  (find id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=))

(defun %model-candidates (task &key (limit 5))
  (let ((scored
          (mapcar (lambda (p) (cons (getf p :id) (%model-score p task)))
                  *model-profiles*)))
    (mapcar #'car
            (subseq (sort scored #'> :key #'cdr) 0 (min limit (length scored))))))

(defun %pick-heuristic-model (prompt)
  (let* ((task (%task-kind prompt))
         (cands (%model-candidates task :limit 1)))
    (or (first cands)
        (getf (first *model-profiles*) :id))))

(defun %planner-enabled-p ()
  (string= (or (config-get-or "model-policy" "planner" "0") "0")
           "1"))

(defun %planner-model ()
  (or (config-get-for "model-policy" "planner-model")
      (%planner-profile-id)))

(defun %now-secs ()
  (get-universal-time))

(defun %starts-with (text prefix)
  (let ((s (or text ""))
        (p (or prefix "")))
    (and (>= (length s) (length p))
         (string= p s :end2 (length p)))))

(defun %clip-model-id (model)
  (if (and model (%starts-with model "cli:"))
      (subseq model 4)
      model))

(defun model-policy-cli-quota-exceeded-p (text)
  (let ((s (string-downcase (or text ""))))
    (some (lambda (p)
            (search (string-downcase p) s :test #'char-equal))
          (or (getf *model-evolution-policy* :cli-quota-patterns)
              '("quota" "rate limit" "cooldown")))))

(defun model-policy-mark-cli-cooloff (cli-id &optional seconds)
  (let ((ttl (or seconds
                 (or (getf *model-evolution-policy* :cli-cooloff-seconds) 3600))))
    (setf (gethash (or cli-id "") *cli-cooloff-until*) (+ (%now-secs) ttl))
    t))

(defun model-policy-actor-stall-threshold ()
  "Ticks with zero output delta before killing an actor. Progress-based, not time-based."
  (max 5 (or (getf *model-evolution-policy* :actor-stall-threshold) 50)))

(defun %cli-on-cooloff-p (cli-id)
  (> (or (gethash (or cli-id "") *cli-cooloff-until*) 0)
     (%now-secs)))

(defun %task-prefers-cli-p (task)
  (member task
          (or (getf *model-evolution-policy* :cli-task-kinds) '())
          :test #'eq))

(defun %truthy-p (value)
  (not (or (null value)
           (eq value :false)
           (eq value :off)
           (eq value :no)
           (and (stringp value)
                (member (string-downcase value) '("0" "false" "off" "no") :test #'string=)))))

(defun %split-model-csv (text)
  (let* ((raw (or text ""))
         (parts '())
         (start 0))
    (loop for i = (position #\, raw :start start)
          do (let ((piece (string-trim '(#\Space #\Tab #\Newline #\Return)
                                       (subseq raw start (or i (length raw))))))
               (when (> (length piece) 0)
                 (push piece parts)))
          if (null i) do (return)
          do (setf start (1+ i)))
    (nreverse parts)))

(defun %active-provider-id ()
  (string-downcase
   (or (and (fboundp 'config-get-for)
            (config-get-for "model-policy" "provider"))
       (getf *model-evolution-policy* :active-provider)
       "openrouter")))

(defun %config-seed-models-for-provider (provider-id)
  (when provider-id
    (let ((raw (and (fboundp 'config-get-for)
                    (config-get-for "model-policy"
                                    (format nil "seed-models-~A" provider-id)))))
      (when (and raw (> (length raw) 0))
        (%split-model-csv raw)))))

(defun %policy-seed-models-for-provider (provider-id)
  (let* ((table (getf *model-evolution-policy* :seed-provider-models))
         (k (and provider-id
                 (ignore-errors (intern (string-upcase provider-id) :keyword)))))
    (or (and k (getf table k))
        (and provider-id (getf table provider-id)))))

(defun %cli-chain-for-task (task)
  (when (%task-prefers-cli-p task)
    (let* ((prefs (or (getf *model-evolution-policy* :cli-preference) '("claude-code" "codex")))
           (found '()))
      (dolist (cli prefs)
        (let ((available (%detect-available-cli (list cli))))
          (when (and available (not (%cli-on-cooloff-p available)))
            (push (format nil "cli:~A" available) found))))
      (%stable-unique-strings (nreverse found)))))

(defun %runtime-vitruvian-signal ()
  (let* ((ctx (and *runtime* (runtime-state-harmonic-context *runtime*)))
         (plan (and ctx (getf ctx :plan)))
         (vit (and plan (getf plan :vitruvian)))
         (fallback (signalograd-routing-vitruvian-min *runtime*)))
    (or (and vit (getf vit :signal))
        fallback)))

(defun %score-entry-for-model (model-id)
  (find model-id (%load-swarm-scores)
        :key (lambda (e) (getf e :model-id))
        :test #'string=))

(defun %profile-latency-score (profile)
  (let ((lat (float (max 1 (getf profile :latency 10)))))
    (max 0.0 (min 1.0 (/ (- 11.0 lat) 10.0)))))

(defun %observed-latency-score (entry profile)
  (let ((lat (and entry (getf entry :latency-ms))))
    (if (and lat (> lat 0))
        (max 0.0 (min 1.0 (- 1.0 (/ (float lat) 8000.0))))
        (%profile-latency-score profile))))

(defun %reasoning-score (profile)
  (max 0.0 (min 1.0
                (/ (+ (float (getf profile :quality 1))
                      (float (getf profile :completion 1)))
                   20.0))))

(defun %seed-score (profile)
  (let* ((entry (%score-entry-for-model (getf profile :id)))
         (weights (or (getf *model-evolution-policy* :seed-weights) '()))
         (w-price (signalograd-routing-weight :price (or (getf weights :price) 0.35) *runtime*))
         (w-speed (signalograd-routing-weight :speed (or (getf weights :speed) 0.20) *runtime*))
         (w-success (signalograd-routing-weight :success (or (getf weights :success) 0.20) *runtime*))
         (w-reasoning (signalograd-routing-weight :reasoning (or (getf weights :reasoning) 0.15) *runtime*))
         (w-vitruvian (or (getf weights :vitruvian) 0.10))
         (weight-sum (max 0.001 (+ w-price w-speed w-success w-reasoning w-vitruvian)))
         (price (/ (%usd-price-score profile) 10.0))
         (speed (%observed-latency-score entry profile))
         (success (or (and entry (getf entry :success-rate)) 0.5))
         (reasoning (%reasoning-score profile))
         (vitruvian (or (and entry (getf entry :vitruvian-signal))
                        (%runtime-vitruvian-signal))))
    (+ (* (/ w-price weight-sum) price)
       (* (/ w-speed weight-sum) speed)
       (* (/ w-success weight-sum) success)
       (* (/ w-reasoning weight-sum) reasoning)
       (* (/ w-vitruvian weight-sum) vitruvian))))

(defun %seed-models ()
  (let* ((provider (%active-provider-id))
         (global-override (and (fboundp 'config-get-for)
                               (config-get-for "model-policy" "seed-models")))
         (global-list (and global-override
                           (> (length global-override) 0)
                           (%split-model-csv global-override)))
         (provider-config (%config-seed-models-for-provider provider))
         (provider-policy (%policy-seed-models-for-provider provider))
         (fallback (getf *model-evolution-policy* :seed-models)))
    (%stable-unique-strings
     (or provider-config
         provider-policy
         global-list
         fallback
         '()))))

(defun %seed-min-samples ()
  (max 1 (or (getf *model-evolution-policy* :seed-min-samples) 3)))

(defun %last-resort-models ()
  (or (getf *model-evolution-policy* :last-resort-models) '()))

(defun %rewrite-capable-models ()
  (or (getf *model-evolution-policy* :rewrite-capable-models)
      (%last-resort-models)))

(defun model-policy-context-summarizer-model ()
  (or (getf *model-evolution-policy* :context-summarizer-model)
      "qwen/qwen3.5-flash-02-23"))

(defun model-policy-context-summarizer-threshold-chars ()
  (max 1024 (or (getf *model-evolution-policy* :context-summarizer-threshold-chars) 12000)))

(defun model-policy-orchestrator-delegate-swarm-p ()
  (%truthy-p (if (getf *model-evolution-policy* :orchestrator-delegate-swarm)
                 (getf *model-evolution-policy* :orchestrator-delegate-swarm)
                 t)))

(defun %task-route-models (task)
  (let* ((routing (%task-routing task))
         (models (and routing (getf routing :models))))
    (when models
      (%stable-unique-strings
       (remove-if-not #'%profile-by-id models)))))

(defun %truth-seeking-models ()
  (%stable-unique-strings
   (append '("x-ai/grok-4.1-fast")
           (or (%task-route-models :truth-seeking) '())
           (%seed-order)
           (%last-resort-models))))

(defun %task-primary-models (task)
  (cond
    ((eq task :critical-reasoning)
     (%rewrite-capable-models))
    ((eq task :memory-ops)
     (or (%task-route-models :memory-ops) (%seed-order)))
    ((eq task :truth-seeking)
     (%truth-seeking-models))
    (t
     (%seed-order))))

(defun %seed-order ()
  (let* ((ids (%seed-models))
         (profiles (remove nil (mapcar #'%profile-by-id ids)))
         (min-samples (%seed-min-samples))
         (cold (some (lambda (p)
                       (let ((entry (%score-entry-for-model (getf p :id))))
                         (< (or (and entry (getf entry :samples))
                              0)
                            min-samples)))
                     profiles)))
    (if cold
        ids
        (mapcar (lambda (p) (getf p :id))
                (sort (copy-list profiles) #'> :key #'%seed-score)))))

(defun %selection-chain (prompt)
  (let* ((task (%task-kind prompt))
         (cli (%cli-chain-for-task task))
         (vit (%runtime-vitruvian-signal))
         (vit-min (signalograd-routing-vitruvian-min *runtime*))
         (primary (%task-primary-models task))
         (fallback (if (eq task :critical-reasoning)
                       '()
                       (%last-resort-models)))
         (harmony-recovery (if (< vit vit-min) (%rewrite-capable-models) '())))
    (or (%stable-unique-strings (append cli primary fallback harmony-recovery))
        (list (getf (first *model-profiles*) :id)))))

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

(defun choose-model (prompt)
  (let ((chain (%selection-chain prompt)))
    (or (first chain)
        (getf (first *model-profiles*) :id))))

;;; --- Upsert Profile ---

(defun model-policy-get ()
  (list :profiles *model-profiles*
        :weights *model-harmony-weights*
        :task-routing *model-task-routing*
        :evolution *model-evolution-policy*))

(defun model-policy-set-weight (metric value)
  (setf (getf *model-harmony-weights* metric) (coerce value 'float))
  (ignore-errors (model-policy-save))
  *model-harmony-weights*)

(defun model-policy-upsert-profile (id &key tier cost latency quality completion tags
                                        usd-in-1k usd-out-1k features)
  (let* ((existing (%profile-by-id id))
         (profile (append
                    (list :id id
                          :tier (or tier (and existing (getf existing :tier)) :custom)
                          :cost (or cost (and existing (getf existing :cost)) 5)
                          :latency (or latency (and existing (getf existing :latency)) 5)
                          :quality (or quality (and existing (getf existing :quality)) 5)
                          :completion (or completion (and existing (getf existing :completion)) 5)
                          :tags (or tags (and existing (getf existing :tags)) '(:custom)))
                    (let ((usd-i (or usd-in-1k (and existing (getf existing :usd-in-1k))))
                          (usd-o (or usd-out-1k (and existing (getf existing :usd-out-1k))))
                          (feats (or features (and existing (getf existing :features)))))
                      (append (when usd-i (list :usd-in-1k usd-i))
                              (when usd-o (list :usd-out-1k usd-o))
                              (when feats (list :features feats)))))))
    (setf *model-profiles*
          (cons profile
                (remove id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=)))
    (ignore-errors (model-policy-save))
    profile))

;;; --- Swarm Evolution ---

(defun swarm-evolve-scores ()
  "Aggregate metrics with exponential decay, persist to swarm_model_scores.sexp."
  (let* ((path (%swarm-scores-path))
         (existing (%load-swarm-scores))
         (decay (or (let ((hp (harmony-policy-get)))
                      (getf (getf hp :swarm) :evolution-decay))
                    0.95)))
    ;; Apply decay to existing scores
    (dolist (entry existing)
      (let ((sr (or (getf entry :success-rate) 0.0))
            (ha (or (getf entry :harmony-avg) 0.0))
            (vs (or (getf entry :vitruvian-signal) 0.0)))
        (setf (getf entry :success-rate) (* sr decay))
        (setf (getf entry :harmony-avg) (* ha decay))
        (setf (getf entry :vitruvian-signal) (* vs decay))))
    ;; Persist
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 existing out)
        (terpri out)))
    existing))

(defun %avg-update (old samples new)
  (if (<= samples 0)
      (float new)
      (/ (+ (* (float old) samples) (float new))
         (1+ samples))))

(defun %save-swarm-scores (scores)
  (let ((path (%swarm-scores-path)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 scores out)
        (terpri out)))
    path))

(defun model-policy-estimate-cost-usd (model prompt response)
  (if (%starts-with (or model "") "cli:")
      0.0
      (let* ((profile (%profile-by-id model))
             (usd-in (or (and profile (getf profile :usd-in-1k)) 0.0))
             (usd-out (or (and profile (getf profile :usd-out-1k)) 0.0))
             (tok-in (max 1.0 (/ (float (length (or prompt ""))) 4.0)))
             (tok-out (max 1.0 (/ (float (length (or response ""))) 4.0))))
        (+ (* usd-in (/ tok-in 1000.0))
           (* usd-out (/ tok-out 1000.0))))))

(defun model-policy-record-outcome (&key model success latency-ms harmony-score cost-usd)
  (let* ((scores (%load-swarm-scores))
         (id (%clip-model-id model))
         (existing (find id scores :key (lambda (e) (getf e :model-id)) :test #'string=))
         (old-n (or (and existing (getf existing :samples)) 0))
         (new-n (1+ old-n))
         (new-sr (%avg-update (or (and existing (getf existing :success-rate)) 0.5)
                              old-n
                              (if success 1.0 0.0)))
         (new-lat (%avg-update (or (and existing (getf existing :latency-ms))
                                   (or latency-ms 0))
                               old-n
                               (or latency-ms 0)))
         (new-ha (%avg-update (or (and existing (getf existing :harmony-avg))
                                  (or harmony-score 0.0))
                              old-n
                              (or harmony-score 0.0)))
         (new-ca (%avg-update (or (and existing (getf existing :cost-avg))
                                  (or cost-usd 0.0))
                              old-n
                              (or cost-usd 0.0)))
         (vit-signal (%runtime-vitruvian-signal))
         (new-vs (%avg-update (or (and existing (getf existing :vitruvian-signal))
                                  vit-signal)
                              old-n
                              vit-signal))
         (entry (list :model-id id
                      :samples new-n
                      :success-rate new-sr
                      :latency-ms new-lat
                      :harmony-avg new-ha
                      :cost-avg new-ca
                      :vitruvian-signal new-vs
                      :last-updated (%now-secs))))
    (%save-swarm-scores
     (cons entry (remove id scores :key (lambda (e) (getf e :model-id)) :test #'string=)))
    entry))

(defun model-policy-selection-trace (prompt chosen chain)
  (let ((task (%task-kind prompt))
        (vit (%runtime-vitruvian-signal)))
    (format nil "task=~A chosen=~A vitruvian=~,3f chain=~{~A~^,~}"
            task chosen vit chain)))

;;; --- Escalation Chain ---

(defun model-escalation-chain (prompt chosen)
  (let ((chain (%selection-chain prompt)))
    (if (member chosen chain :test #'string=)
        (or (member chosen chain :test #'string=) chain)
        chain)))
