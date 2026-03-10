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

(declaim (ftype function backend-complete model-policy-get))

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
  (string= (or (config-get-or "model-policy" "planner" "1") "1")
           "1"))

(defun %planner-model ()
  (or (config-get-for "model-policy" "planner-model")
      (%planner-profile-id)))

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
  (let ((task (%task-kind prompt)))
    (case task
      (:software-dev (%software-dev-choose prompt))
      (:memory-ops (%memory-ops-choose))
      (t (%choose-model-default prompt)))))

;;; --- Upsert Profile ---

(defun model-policy-get ()
  (list :profiles *model-profiles*
        :weights *model-harmony-weights*
        :task-routing *model-task-routing*))

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
            (ha (or (getf entry :harmony-avg) 0.0)))
        (setf (getf entry :success-rate) (* sr decay))
        (setf (getf entry :harmony-avg) (* ha decay))))
    ;; Persist
    (ensure-directories-exist path)
    (with-open-file (out path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*print-pretty* t))
        (prin1 existing out)
        (terpri out)))
    existing))

;;; --- Escalation Chain ---

(defun model-escalation-chain (prompt chosen)
  (let* ((task (%task-kind prompt))
         (ordered (%model-candidates task :limit 6))
         (safe-models
           (mapcar (lambda (p) (getf p :id))
                   (subseq (%profiles-by-cost) 0 (min 3 (length *model-profiles*))))))
    (let ((chain (append ordered safe-models)))
      (setf chain (remove-duplicates chain :test #'string=))
      (if (member chosen chain :test #'string=)
          (let ((tail (member chosen chain :test #'string=)))
            (if tail tail chain))
          chain))))
