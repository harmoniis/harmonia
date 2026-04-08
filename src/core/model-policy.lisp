;;; model-policy.lisp — Harmonic model selection policy: core state, task classification.

(in-package :harmonia)

;; Forward declarations for variables defined in model-routing.lisp (loaded later).
(declaim (special *routing-tier* *last-task-kind* *routing-rules-sexp*))

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
                                (%tmpdir-state-root))))
                  (concatenate 'string root "/model-policy.sexp"))))))

(defparameter *model-profiles* '())
(defparameter *model-harmony-weights* '())
(defparameter *model-task-routing* '())
(defparameter *default-model-evolution-policy*
  '(:seed-models ()
    :seed-provider-models ()
    :active-provider "unified"
    :seed-weights (:price 0.35 :speed 0.20 :success 0.20 :reasoning 0.15 :vitruvian 0.10)
    :seed-min-samples 3
    :last-resort-models ("x-ai/grok-4.1-fast"
                         "qwen/qwen3.6-plus:free"
                         "anthropic/claude-opus-4.6")
    :rewrite-capable-models ("anthropic/claude-opus-4.6"
                             "x-ai/grok-4.20")
    :cli-preference ("claude-code" "codex")
    :cli-task-kinds (:software-dev :coding :critical-reasoning)
    :actor-stall-threshold 180
    :cli-cooloff-seconds 3600
    :cli-quota-patterns ("quota" "rate limit" "cooldown" "usage cap" "too many requests")
    :vitruvian-signal-min 0.62
    :context-summarizer-model "qwen/qwen3.5-flash-02-23"
    :context-summarizer-threshold-chars 12000
    :orchestrator-delegate-swarm t))
(defparameter *model-evolution-policy* (copy-tree *default-model-evolution-policy*))
(defparameter *cli-cooloff-until* (make-hash-table :test 'equal))

(declaim (ftype function backend-complete model-policy-get))

;;; --- Task Classification ---

(defparameter *truth-seeking-keywords-fallback*
  '("truth" "reality" "accurate" "accuracy" "fact check" "fact-check"
    "verify" "verification" "debunk" "controvers" "what actually"
    "what is really" "real-time" "realtime" "current event" "harmonic truth"))

(defun %truth-seeking-prompt-p (prompt)
  (let ((p (string-downcase (or prompt "")))
        (keywords (or (when (fboundp 'load-security-pattern)
                        (funcall 'load-security-pattern :truth-seeking-keywords))
                      *truth-seeking-keywords-fallback*)))
    (some (lambda (kw) (search (string-downcase kw) p)) keywords)))

(defun %question-marker-p (prompt)
  "Return T if PROMPT looks like a question rather than an action request."
  (let ((p (string-downcase (or prompt ""))))
    (or (search "?" p)
        (search "what is" p) (search "what are" p) (search "what does" p)
        (search "how does" p) (search "how do" p) (search "how is" p)
        (search "tell me" p) (search "explain" p) (search "describe" p)
        (search "do you" p) (search "can you" p) (search "do we" p)
        (search "show me" p) (search "list " p) (search "status" p)
        (search "who " p) (search "why " p) (search "where " p))))

(defun %task-kind (prompt)
  (let* ((p (string-downcase prompt))
         (is-question (%question-marker-p p)))
    (cond
      ;; Action-oriented software dev -- must have action verbs, not just keywords
      ((and (not is-question)
            (or (search "implement" p) (search "refactor" p) (search "write code" p)
                (search "fix bug" p) (search "pull request" p) (search "pr " p)
                (search "commit" p) (search "deploy" p) (search "test suite" p)
                (search "debug" p) (search "build" p) (search "compile error" p)))
       :software-dev)
      ;; Memory operations -- only when actually doing ops, not asking about memory system
      ((and (not is-question)
            (or (search "summarize" p) (search "compress" p)
                (search "digest" p) (search "consolidate" p) (search "journal" p))
            ;; "memory" alone is too broad -- require action context
            (not (search "memory system" p))
            (not (search "how memory" p)))
       :memory-ops)
      ;; Truth-seeking -- web search, fact checking, controversial topics
      ((%truth-seeking-prompt-p p) :truth-seeking)
      ;; Codemode -- pipeline/batch operations
      ((or (search "codemode" p) (search "mcp" p) (search "pipeline" p)
           (search "batch tools" p) (search "tool chain" p))
       :codemode)
      ;; Vision -- only when actually processing images
      ((and (not is-question)
            (or (search "ocr" p) (search "image" p) (search "vision" p)))
       :vision)
      ;; Critical reasoning -- only for actual rewrite/evolution actions
      ((and (not is-question)
            (or (search "rewrite" p) (search "evolution" p)))
       :critical-reasoning)
      ;; Planning -- only for actual planning tasks, not questions about orchestration
      ((and (not is-question)
            (or (search "plan" p) (search "decision" p)))
       :planning)
      ;; Questions about architecture/orchestration -> general (orchestrator answers)
      ((and is-question
            (or (search "orchestrat" p) (search "architect" p)
                (search "harmoni" p) (search "system" p)
                (search "swarm" p) (search "subagent" p)
                (search "signalograd" p) (search "conductor" p)))
       :general)
      ;; Tooling
      ((or (search "tool op=" p) (search "send" p) (search "search " p)) :tooling)
      ;; Coding -- only with action intent
      ((and (not is-question)
            (or (search "code" p) (search "bug" p)))
       :coding)
      (t :general))))

;;; --- Task Weights (signalograd-adaptive) ---

(defun %task-weights-base (task)
  "Base weight distributions per task kind. These are starting points --
   signalograd applies adaptive deltas at runtime."
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

(defun %task-weights (task)
  "Return signalograd-adaptive task weights. Base weights are adjusted by
   signalograd routing deltas, then re-normalized so they sum to 1.0."
  (let* ((base (%task-weights-base task))
         ;; Map task-weight dimensions to signalograd routing metrics
         ;; completion -> :reasoning, correctness -> :success, speed -> :speed, price -> :price
         (w-completion (signalograd-routing-weight :reasoning
                         (or (getf base :completion) 0.0) *runtime*))
         (w-correctness (signalograd-routing-weight :success
                          (or (getf base :correctness) 0.0) *runtime*))
         (w-speed (signalograd-routing-weight :speed
                    (or (getf base :speed) 0.0) *runtime*))
         (w-price (signalograd-routing-weight :price
                    (or (getf base :price) 0.0) *runtime*))
         ;; No signalograd mapping -- use base values directly
         (w-token-eff (or (getf base :token-efficiency) 0.0))
         (w-orch-eff (or (getf base :orchestration-efficiency) 0.0))
         (w-experience (or (getf base :experience) 0.0))
         ;; Re-normalize to sum to 1.0
         (total (max 0.001 (+ w-completion w-correctness w-speed w-price
                              w-token-eff w-orch-eff w-experience))))
    (list :completion (/ w-completion total)
          :correctness (/ w-correctness total)
          :speed (/ w-speed total)
          :price (/ w-price total)
          :token-efficiency (/ w-token-eff total)
          :orchestration-efficiency (/ w-orch-eff total)
          :experience (/ w-experience total))))

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
