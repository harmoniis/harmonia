(:weights (:completion 0.30 :correctness 0.20 :speed 0.12 :price 0.12
           :token-efficiency 0.10 :orchestration-efficiency 0.10 :experience 0.06)
 :profiles
 (;; Primary REPL-capable models (can speak s-expressions)
  (:id "google/gemini-2.5-flash-lite-preview-09-2025" :tier :free :cost 0 :latency 1 :quality 6 :completion 6
   :usd-in-1k 0.000025 :usd-out-1k 0.0001
   :tags (:free :fast :token-efficient :memory-ops :reasoning :structured-output :sexp-capable))
  (:id "inception/mercury-2" :tier :lite :cost 1 :latency 2 :quality 5 :completion 5
   :usd-in-1k 0.00012 :usd-out-1k 0.00048
   :tags (:cheap :fast :reasoning :token-efficient :planning :software-dev))
  (:id "qwen/qwen3.6-plus:free" :tier :free :cost 0 :latency 2 :quality 5 :completion 5
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:free :fast :reasoning :token-efficient :memory-ops))
  (:id "qwen/qwen3.5-flash-02-23" :tier :lite :cost 1 :latency 2 :quality 4 :completion 4
   :usd-in-1k 0.00010 :usd-out-1k 0.00040
   :tags (:cheap :fast :token-efficient :memory-ops :reasoning))
  (:id "minimax/minimax-m2.5" :tier :lite :cost 2 :latency 2 :quality 4 :completion 4
   :usd-in-1k 0.0002 :usd-out-1k 0.0011
   :tags (:cheap :fast :token-efficient :memory-ops))
  ;; Eco — smart + fast
  (:id "x-ai/grok-4.1-fast" :tier :fast-smart :cost 5 :latency 2 :quality 7 :completion 7
   :usd-in-1k 0.0002 :usd-out-1k 0.0005
   :features (:reasoning t :web-search t :x-search t :truth-seeking t)
   :native-tools
     (:reasoning (:enabled t :effort "high" :exclude t)
      :web-search (:plugin-id "web" :engine "native" :search-context-size "high")
      :x-search (:plugin-id "web" :engine "native" :search-context-size "high"))
   :tags (:truth-seeking :fast :strong :web-search :x-search :realtime))
  (:id "amazon/nova-micro-v1" :tier :micro :cost 1 :latency 1 :quality 2 :completion 2
   :usd-in-1k 0.000035 :usd-out-1k 0.00014
   :tags (:cheap :fast :routing :token-efficient :tool-use))
  (:id "amazon/nova-lite-v1" :tier :lite :cost 2 :latency 2 :quality 3 :completion 3
   :usd-in-1k 0.00006 :usd-out-1k 0.00024
   :tags (:cheap :vision :ocr :token-efficient))
  ;; Premium — highest intelligence
  (:id "x-ai/grok-4.20" :tier :frontier :cost 10 :latency 6 :quality 10 :completion 10
   :usd-in-1k 0.010 :usd-out-1k 0.030
   :features (:reasoning t :web-search t :truth-seeking t)
   :tags (:frontier :reasoning :truth-seeking :software-dev :codemode))
  (:id "anthropic/claude-opus-4.6" :tier :frontier :cost 10 :latency 7 :quality 10 :completion 10
   :usd-in-1k 0.015 :usd-out-1k 0.075
   :tags (:frontier :reasoning :writing :codemode :software-dev))
  ;; Demoted — cannot speak s-expressions, unreliable downtime
  (:id "ber1-ai/qwen3.5-27b" :tier :demoted :cost 0 :latency 4 :quality 3 :completion 3
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :features (:reasoning t)
   :tags (:demoted :unreliable :no-sexp))
  (:id "ber1-ai/magistral-24b" :tier :demoted :cost 0 :latency 5 :quality 2 :completion 2
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:demoted :unreliable :no-sexp))
  (:id "ber1-ai/nanbeige-3b" :tier :demoted :cost 0 :latency 1 :quality 1 :completion 1
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:demoted :unreliable :no-sexp)))
 :task-routing
 (:software-dev (:models ("anthropic/claude-opus-4.6" "x-ai/grok-4.20"
                           "x-ai/grok-4.1-fast"
                           "google/gemini-2.5-flash-lite-preview-09-2025")
                  :cli-preference ("claude-code" "codex")
                  :openrouter-fallback t)
  :memory-ops (:models ("google/gemini-2.5-flash-lite-preview-09-2025"
                         "qwen/qwen3.6-plus:free" "qwen/qwen3.5-flash-02-23"
                         "inception/mercury-2"))
  :truth-seeking (:models ("x-ai/grok-4.1-fast" "x-ai/grok-4.20"
                           "anthropic/claude-opus-4.6")
                  :openrouter-fallback t)
  :casual (:models ("google/gemini-2.5-flash-lite-preview-09-2025"
                     "qwen/qwen3.6-plus:free" "inception/mercury-2"))
  :general (:models ("google/gemini-2.5-flash-lite-preview-09-2025"
                      "qwen/qwen3.6-plus:free" "inception/mercury-2"
                      "x-ai/grok-4.1-fast")))
 :evolution
 (:seed-models ()
  :active-provider "unified"
  :seed-provider-models
  (:openrouter ("google/gemini-2.5-flash-lite-preview-09-2025"
                "qwen/qwen3.6-plus:free" "inception/mercury-2"
                "x-ai/grok-4.1-fast"
                "qwen/qwen3.5-flash-02-23" "minimax/minimax-m2.5")
   :xai ("x-ai/grok-4.20")
   :anthropic ("anthropic/claude-opus-4.6")
   :google-ai-studio ("google/gemini-2.5-flash-lite-preview-09-2025")
   :google-vertex ("google/gemini-2.5-flash-lite-preview-09-2025")
   :bedrock ("amazon/nova-micro-v1" "amazon/nova-lite-v1")
   :groq ("qwen/qwen3.6-plus:free")
   :alibaba ("qwen/qwen3.6-plus:free"))
  :seed-weights (:price 0.35 :speed 0.20 :success 0.20 :reasoning 0.15 :vitruvian 0.10)
  :seed-min-samples 3
  :last-resort-models ("google/gemini-2.5-flash-lite-preview-09-2025"
                       "x-ai/grok-4.1-fast"
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
  :orchestrator-delegate-swarm t
  :orchestrator-enabled t)
 :routing-rules
 (:version 1
  :task-tier-hints
    ((:task :memory-ops :preferred-tier :eco)
     (:task :critical-reasoning :preferred-tier :premium)
     (:task :truth-seeking :preferred-tier :auto))
  :model-bans ("ber1-ai/qwen3.5-27b" "ber1-ai/magistral-24b" "ber1-ai/nanbeige-3b")
  :model-boosts ("google/gemini-2.5-flash-lite-preview-09-2025")
  :cascade-config (:max-escalations 3 :confidence-threshold 0.7)))
