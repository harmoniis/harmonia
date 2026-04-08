;;; model-providers.lisp — Provider preferences, backend configuration, scoring, outcome recording.

(in-package :harmonia)

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
    (let ((*read-eval* nil)) (read in nil nil))))

(defun %model-policy-load-source ()
  (let ((state-path (%model-policy-resolve-state-path)))
    (cond
      ((probe-file state-path) (%model-policy-read-file state-path))
      ((probe-file *model-policy-config-path*) (%model-policy-read-file *model-policy-config-path*))
      (t (error "model policy config missing: ~A" *model-policy-config-path*)))))

(defun model-policy-load ()
  (let ((src (%model-policy-load-source)))
    (setf *model-profiles* (copy-tree (getf src :profiles))
          *model-harmony-weights* (copy-tree (getf src :weights))
          *model-task-routing* (copy-tree (getf src :task-routing))
          *model-evolution-policy*
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
      (let ((*print-pretty* t)) (prin1 (model-policy-get) out) (terpri out)))
    path))

(defun %model-features (model-id)
  (let ((profile (%profile-by-id model-id)))
    (when profile (getf profile :features))))

(defun %model-native-tools (model-id)
  (let ((profile (%profile-by-id model-id)))
    (when profile (getf profile :native-tools))))

(defun model-has-native-tool-p (model-id tool-key)
  (let ((tools (%model-native-tools model-id)))
    (and tools (not (null (getf tools tool-key))))))

(defun model-feature-params (model-id)
  (let ((feats (%model-features model-id)))
    (if feats (format nil "~S" feats) "()")))

(defun %usd-price-score (profile)
  "Score 0-10 based on USD pricing. Cheaper = higher score."
  (let ((usd-in (getf profile :usd-in-1k))
        (usd-out (getf profile :usd-out-1k)))
    (if (and usd-in usd-out)
        (let ((avg (/ (+ usd-in usd-out) 2.0)))
          (cond ((<= avg 0.0) 10.0) ((<= avg 0.001) 8.0) ((<= avg 0.005) 6.0)
                ((<= avg 0.02) 4.0) (t 2.0)))
        (/ 10.0 (max 1 (getf profile :cost 10))))))

(defun %profile-by-id (id)
  (find id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=))

(defun %model-score (profile task)
  (let* ((weights (%task-weights task))
         (tags (getf profile :tags))
         (need (%task-need task))
         (fit (loop for n in need count (member n tags))))
    (+ (* (%model-weight weights :completion) (getf profile :completion 1))
       (* (%model-weight weights :correctness) (getf profile :quality 1))
       (* (%model-weight weights :speed) (/ 10.0 (max 1 (getf profile :latency 10))))
       (* (%model-weight weights :price) (%usd-price-score profile))
       (* (%model-weight weights :token-efficiency)
          (max 0.0 (min 1.0 (/ 10.0 (+ (getf profile :cost 10) (* 0.5 (getf profile :latency 10)))))))
       (* (%model-weight weights :orchestration-efficiency)
          (%tag-hit-score tags :codemode :tool-use))
       (* (%model-weight weights :experience) (%experience-score (getf profile :id)))
       (* 0.5 fit))))

(defun %model-candidates (task &key (limit 5))
  (let ((scored (mapcar (lambda (p) (cons (getf p :id) (%model-score p task)))
                        *model-profiles*)))
    (mapcar #'car (subseq (sort scored #'> :key #'cdr) 0 (min limit (length scored))))))

(defun %pick-heuristic-model (prompt)
  (or (first (%model-candidates (%task-kind prompt) :limit 1))
      (getf (first *model-profiles*) :id)))

(defun %planner-enabled-p ()
  (string= (or (config-get-or "model-policy" "planner" "0") "0") "1"))

(defun %planner-model ()
  (or (config-get-for "model-policy" "planner-model") (%planner-profile-id)))

(defun %now-secs () (get-universal-time))

(defun %starts-with (text prefix)
  (let ((s (or text "")) (p (or prefix "")))
    (and (>= (length s) (length p)) (string= p s :end2 (length p)))))

(defun %clip-model-id (model)
  (if (and model (%starts-with model "cli:")) (subseq model 4) model))

(defun model-policy-cli-quota-exceeded-p (text)
  (let ((s (string-downcase (or text ""))))
    (some (lambda (p) (search (string-downcase p) s :test #'char-equal))
          (or (getf *model-evolution-policy* :cli-quota-patterns)
              '("quota" "rate limit" "cooldown")))))

(defun model-policy-mark-cli-cooloff (cli-id &optional seconds)
  (setf (gethash (or cli-id "") *cli-cooloff-until*)
        (+ (%now-secs) (or seconds (or (getf *model-evolution-policy* :cli-cooloff-seconds) 3600))))
  t)

(defun model-policy-actor-stall-threshold ()
  "Ticks with zero output delta before killing an actor."
  (max 5 (or (getf *model-evolution-policy* :actor-stall-threshold) 50)))

(defun %cli-on-cooloff-p (cli-id)
  (> (or (gethash (or cli-id "") *cli-cooloff-until*) 0) (%now-secs)))

(defun %task-prefers-cli-p (task)
  (member task (or (getf *model-evolution-policy* :cli-task-kinds) '()) :test #'eq))

(defun %truthy-p (value)
  (not (or (null value) (eq value :false) (eq value :off) (eq value :no)
           (and (stringp value)
                (member (string-downcase value) '("0" "false" "off" "no") :test #'string=)))))

(defun %split-model-csv (text)
  (let ((raw (or text "")) (parts '()) (start 0))
    (loop for i = (position #\, raw :start start)
          do (let ((piece (string-trim '(#\Space #\Tab #\Newline #\Return)
                                       (subseq raw start (or i (length raw))))))
               (when (> (length piece) 0) (push piece parts)))
          if (null i) do (return)
          do (setf start (1+ i)))
    (nreverse parts)))

(defun %active-provider-id ()
  (string-downcase
   (or (and (fboundp 'config-get-for) (config-get-for "model-policy" "provider"))
       (getf *model-evolution-policy* :active-provider)
       "openrouter")))

(defun %config-seed-models-for-provider (provider-id)
  (when provider-id
    (let ((raw (and (fboundp 'config-get-for)
                    (config-get-for "model-policy" (format nil "seed-models-~A" provider-id)))))
      (when (and raw (> (length raw) 0)) (%split-model-csv raw)))))

(defun %policy-seed-models-for-provider (provider-id)
  (let* ((table (getf *model-evolution-policy* :seed-provider-models))
         (k (and provider-id (handler-case (intern (string-upcase provider-id) :keyword) (error () nil)))))
    (or (and k (getf table k))
        (and provider-id (getf table provider-id)))))

(defun %cli-chain-for-task (task)
  (when (%task-prefers-cli-p task)
    (let ((found '()))
      (dolist (cli (or (getf *model-evolution-policy* :cli-preference) '("claude-code" "codex")))
        (let ((available (%detect-available-cli (list cli))))
          (when (and available (not (%cli-on-cooloff-p available)))
            (push (format nil "cli:~A" available) found))))
      (%stable-unique-strings (nreverse found)))))

(defun %runtime-vitruvian-signal ()
  (let* ((ctx (and *runtime* (runtime-state-harmonic-context *runtime*)))
         (vit (and ctx (getf (getf ctx :plan) :vitruvian))))
    (or (and vit (getf vit :signal))
        (signalograd-routing-vitruvian-min *runtime*))))

(defun %swarm-scores-path ()
  (concatenate 'string
               (or (config-get-for "model-policy" "state-root" "global") (%tmpdir-state-root))
               "/swarm_model_scores.sexp"))

(defun %load-swarm-scores ()
  (let ((path (%swarm-scores-path)))
    (if (probe-file path)
        (with-open-file (in path :direction :input)
          (let ((*read-eval* nil)) (read in nil '())))
        '())))

(defun %experience-score (model-id)
  "Success-rate * harmony-avg if >= 3 samples, else 0.0."
  (let ((entry (find model-id (%load-swarm-scores)
                     :key (lambda (e) (getf e :model-id)) :test #'string=)))
    (if (and entry (>= (or (getf entry :samples) 0) 3))
        (* (or (getf entry :success-rate) 0.0) (or (getf entry :harmony-avg) 0.0))
        0.0)))

(defun %score-entry-for-model (model-id)
  (find model-id (%load-swarm-scores)
        :key (lambda (e) (getf e :model-id)) :test #'string=))

(defun %profile-latency-score (profile)
  (max 0.0 (min 1.0 (/ (- 11.0 (float (max 1 (getf profile :latency 10)))) 10.0))))

(defun %observed-latency-score (entry profile)
  (let ((lat (and entry (getf entry :latency-ms))))
    (if (and lat (> lat 0))
        (max 0.0 (min 1.0 (- 1.0 (/ (float lat) 8000.0))))
        (%profile-latency-score profile))))

(defun %reasoning-score (profile)
  (max 0.0 (min 1.0 (/ (+ (float (getf profile :quality 1))
                           (float (getf profile :completion 1))) 20.0))))

(defun %seed-score (profile)
  (let* ((entry (%score-entry-for-model (getf profile :id)))
         (weights (or (getf *model-evolution-policy* :seed-weights) '()))
         (w-price (signalograd-routing-weight :price (or (getf weights :price) 0.35) *runtime*))
         (w-speed (signalograd-routing-weight :speed (or (getf weights :speed) 0.20) *runtime*))
         (w-success (signalograd-routing-weight :success (or (getf weights :success) 0.20) *runtime*))
         (w-reasoning (signalograd-routing-weight :reasoning (or (getf weights :reasoning) 0.15) *runtime*))
         (w-vitruvian (or (getf weights :vitruvian) 0.10))
         (weight-sum (max 0.001 (+ w-price w-speed w-success w-reasoning w-vitruvian))))
    (+ (* (/ w-price weight-sum) (/ (%usd-price-score profile) 10.0))
       (* (/ w-speed weight-sum) (%observed-latency-score entry profile))
       (* (/ w-success weight-sum) (or (and entry (getf entry :success-rate)) 0.5))
       (* (/ w-reasoning weight-sum) (%reasoning-score profile))
       (* (/ w-vitruvian weight-sum)
          (or (and entry (getf entry :vitruvian-signal)) (%runtime-vitruvian-signal))))))

(defun %seed-models ()
  (let* ((provider (%active-provider-id))
         (global-override (and (fboundp 'config-get-for)
                               (config-get-for "model-policy" "seed-models")))
         (global-list (and global-override (> (length global-override) 0)
                           (%split-model-csv global-override))))
    (%stable-unique-strings
     (or (%config-seed-models-for-provider provider)
         (%policy-seed-models-for-provider provider)
         global-list
         (getf *model-evolution-policy* :seed-models)
         '()))))

(defun %seed-min-samples ()
  (max 1 (or (getf *model-evolution-policy* :seed-min-samples) 3)))

(defun %last-resort-models ()
  (or (getf *model-evolution-policy* :last-resort-models) '()))

(defun %rewrite-capable-models ()
  (or (getf *model-evolution-policy* :rewrite-capable-models) (%last-resort-models)))

(defun model-policy-context-summarizer-model ()
  (or (getf *model-evolution-policy* :context-summarizer-model) "qwen/qwen3.5-flash-02-23"))

(defun model-policy-context-summarizer-threshold-chars ()
  (max 1024 (or (getf *model-evolution-policy* :context-summarizer-threshold-chars) 12000)))

(defun model-policy-orchestrator-delegate-swarm-p ()
  (%truthy-p (if (getf *model-evolution-policy* :orchestrator-delegate-swarm)
                 (getf *model-evolution-policy* :orchestrator-delegate-swarm) t)))

(defun model-policy-orchestrator-model ()
  "Select orchestrator model from tier pool -- no hardcoded models."
  (or (getf *model-evolution-policy* :orchestrator-model)
      (first (%tier-model-pool *routing-tier*))
      (first (%tier-model-pool :auto))
      ""))

(defun model-policy-orchestrator-enabled-p ()
  (%truthy-p (getf *model-evolution-policy* :orchestrator-enabled)))

(defun %task-route-models (task)
  (let ((models (and (getf (%task-routing task) :models))))
    (when models (%stable-unique-strings (remove-if-not #'%profile-by-id models)))))

(defun %truth-seeking-models ()
  (%stable-unique-strings
   (append (mapcar (lambda (p) (getf p :id))
                   (remove-if-not (lambda (p) (getf (getf p :features) :truth-seeking))
                                  *model-profiles*))
           (or (%task-route-models :truth-seeking) '())
           (%seed-order)
           (%last-resort-models))))

(defun %task-primary-models (task)
  (cond
    ((eq task :critical-reasoning) (%rewrite-capable-models))
    ((eq task :memory-ops) (or (%task-route-models :memory-ops) (%seed-order)))
    ((eq task :truth-seeking) (%truth-seeking-models))
    (t (%seed-order))))

(defun %seed-order ()
  (let* ((ids (%seed-models))
         (profiles (remove nil (mapcar #'%profile-by-id ids)))
         (min-samples (%seed-min-samples))
         (cold (some (lambda (p)
                       (let ((entry (%score-entry-for-model (getf p :id))))
                         (< (or (and entry (getf entry :samples)) 0) min-samples)))
                     profiles)))
    (if cold ids
        (mapcar (lambda (p) (getf p :id))
                (sort (copy-list profiles) #'> :key #'%seed-score)))))

(defun %orchestrator-classify (prompt)
  "Call mercury-2 to classify task and select model. Falls back to %selection-chain."
  (handler-case
      (let* ((available-models
               (with-output-to-string (out)
                 (dolist (p *model-profiles*)
                   (format out "~A tags=~{~A~^,~}~%"
                           (getf p :id)
                           (mapcar #'string-downcase
                                   (mapcar #'symbol-name (getf p :tags)))))))
             (classify-prompt
               (format nil (load-prompt :evolution :task-classifier nil
                             "You are a task classifier. Given the user prompt and available models, output exactly one line:
TASK_KIND=<kind> MODEL=<model-id>

Rules:
- x-ai/grok ONLY for truth-seeking or controversial topics
- minimax for fast reasoning
- cli:claude-code for software-dev tasks
- inception/mercury for general/planning tasks
Available models: ~A
User prompt: ~A")
                       available-models prompt))
             (response (string-trim '(#\Space #\Newline #\Tab)
                                    (backend-complete classify-prompt
                                                     (model-policy-orchestrator-model)))))
        (let ((task-pos (search "TASK_KIND=" response :test #'char-equal))
              (model-pos (search "MODEL=" response :test #'char-equal)))
          (if (and task-pos model-pos)
              (let* ((task-start (+ task-pos 10))
                     (task-end (or (position #\Space response :start task-start) (length response)))
                     (task-str (string-trim '(#\Space) (subseq response task-start task-end)))
                     (model-start (+ model-pos 6))
                     (model-end (or (position #\Space response :start model-start)
                                    (position #\Newline response :start model-start)
                                    (length response)))
                     (model-id (string-trim '(#\Space) (subseq response model-start model-end)))
                     (task-kw (handler-case (intern (string-upcase task-str) :keyword) (error () nil))))
                (if (and task-kw (or (%profile-by-id model-id) (%starts-with model-id "cli:")))
                    (values task-kw model-id)
                    (values (%task-kind prompt) (first (%selection-chain prompt)))))
              (values (%task-kind prompt) (first (%selection-chain prompt))))))
    (error (_) (declare (ignore _))
      (values (%task-kind prompt) (first (%selection-chain prompt))))))

(defun %selection-chain (prompt)
  (let* ((task (%task-kind prompt))
         (cli (%cli-chain-for-task task))
         (vit (%runtime-vitruvian-signal))
         (vit-min (signalograd-routing-vitruvian-min *runtime*))
         (primary (let ((raw (%task-primary-models task)))
                    (if (eq task :truth-seeking) raw
                        (remove-if (lambda (m)
                                     (and (model-has-native-tool-p m :web-search)
                                          (getf (getf (%profile-by-id m) :features) :truth-seeking)))
                                   raw))))
         (fallback (if (eq task :critical-reasoning) '() (%last-resort-models)))
         (harmony-recovery (if (< vit vit-min) (%rewrite-capable-models) '())))
    (or (%stable-unique-strings (append cli primary fallback harmony-recovery))
        (list (getf (first *model-profiles*) :id)))))

(defun model-policy-get ()
  (list :profiles *model-profiles*
        :weights *model-harmony-weights*
        :task-routing *model-task-routing*
        :evolution *model-evolution-policy*))

(defun model-policy-set-weight (metric value)
  (setf (getf *model-harmony-weights* metric) (coerce value 'float))
  (handler-case (model-policy-save) (error () nil))
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
          (cons profile (remove id *model-profiles* :key (lambda (p) (getf p :id)) :test #'string=)))
    (handler-case (model-policy-save) (error () nil))
    profile))

(defun swarm-evolve-scores ()
  "Aggregate metrics with exponential decay, persist to swarm_model_scores.sexp."
  (let* ((path (%swarm-scores-path))
         (existing (%load-swarm-scores))
         (decay (or (let ((hp (harmony-policy-get)))
                      (getf (getf hp :swarm) :evolution-decay)) 0.95)))
    (dolist (entry existing)
      (setf (getf entry :success-rate) (* (or (getf entry :success-rate) 0.0) decay)
            (getf entry :harmony-avg) (* (or (getf entry :harmony-avg) 0.0) decay)
            (getf entry :vitruvian-signal) (* (or (getf entry :vitruvian-signal) 0.0) decay)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t)) (prin1 existing out) (terpri out)))
    existing))

(defun %avg-update (old samples new)
  (if (<= samples 0) (float new)
      (/ (+ (* (float old) samples) (float new)) (1+ samples))))

(defun %save-swarm-scores (scores)
  (let ((path (%swarm-scores-path)))
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t)) (prin1 scores out) (terpri out)))
    path))

(defun model-policy-estimate-cost-usd (model prompt response)
  (if (%starts-with (or model "") "cli:")
      0.0
      (let* ((profile (%profile-by-id model))
             (usd-in (or (and profile (getf profile :usd-in-1k)) 0.0))
             (usd-out (or (and profile (getf profile :usd-out-1k)) 0.0))
             (tok-in (max 1.0 (/ (float (length (or prompt ""))) 4.0)))
             (tok-out (max 1.0 (/ (float (length (or response ""))) 4.0))))
        (+ (* usd-in (/ tok-in 1000.0)) (* usd-out (/ tok-out 1000.0))))))

(defun model-policy-record-outcome (&key model success latency-ms harmony-score cost-usd)
  (let* ((scores (%load-swarm-scores))
         (id (%clip-model-id model))
         (existing (find id scores :key (lambda (e) (getf e :model-id)) :test #'string=))
         (old-n (or (and existing (getf existing :samples)) 0))
         (vit-signal (%runtime-vitruvian-signal))
         (entry (list :model-id id
                      :samples (1+ old-n)
                      :success-rate (%avg-update (or (and existing (getf existing :success-rate)) 0.5)
                                                 old-n (if success 1.0 0.0))
                      :latency-ms (%avg-update (or (and existing (getf existing :latency-ms))
                                                   (or latency-ms 0))
                                               old-n (or latency-ms 0))
                      :harmony-avg (%avg-update (or (and existing (getf existing :harmony-avg))
                                                    (or harmony-score 0.0))
                                                old-n (or harmony-score 0.0))
                      :cost-avg (%avg-update (or (and existing (getf existing :cost-avg))
                                                 (or cost-usd 0.0))
                                             old-n (or cost-usd 0.0))
                      :vitruvian-signal (%avg-update (or (and existing (getf existing :vitruvian-signal))
                                                        vit-signal)
                                                     old-n vit-signal)
                      :last-updated (%now-secs))))
    (%save-swarm-scores
     (cons entry (remove id scores :key (lambda (e) (getf e :model-id)) :test #'string=)))
    (%route-feedback-to-actor id success latency-ms cost-usd)
    entry))

(defun %route-feedback-to-actor (model-id success latency-ms cost-usd)
  "Send route feedback to RouterActor via IPC for per-tier statistics."
  (handler-case

      (when (fboundp 'ipc-call)
      (ipc-call (%sexp-to-ipc-string
                 `(:component "router" :op "signal" :payload
                   ,(%sexp-to-ipc-string
                     `(:route-feedback :model ,model-id
                       :task ,(symbol-name (or *last-task-kind* :general)

    (error () nil))
                       :tier ,(symbol-name (or *routing-tier* :auto))
                       :success ,(if success t nil)
                       :latency-ms ,(or latency-ms 0) :cost-usd ,(or cost-usd 0.0)))))))))

(defun model-policy-selection-trace (prompt chosen chain)
  (format nil "task=~A chosen=~A vitruvian=~,3f chain=~{~A~^,~}"
          (%task-kind prompt) chosen (%runtime-vitruvian-signal) chain))
