(:weights (:completion 0.30 :correctness 0.20 :speed 0.12 :price 0.12
           :token-efficiency 0.10 :orchestration-efficiency 0.10 :experience 0.06)
 :profiles
 ((:id "amazon/nova-micro-v1" :tier :micro :cost 1 :latency 1 :quality 2 :completion 2
   :usd-in-1k 0.000035 :usd-out-1k 0.00014
   :tags (:cheap :fast :routing :token-efficient :tool-use))
  (:id "amazon/nova-lite-v1" :tier :lite :cost 2 :latency 2 :quality 3 :completion 3
   :usd-in-1k 0.00006 :usd-out-1k 0.00024
   :tags (:cheap :vision :ocr :token-efficient))
  (:id "amazon/nova-pro-v1" :tier :pro :cost 4 :latency 3 :quality 5 :completion 5
   :usd-in-1k 0.0008 :usd-out-1k 0.0032
   :tags (:balanced :reasoning :tooling :tool-use))
  (:id "qwen/qwen3-coder" :tier :lite :cost 2 :latency 3 :quality 4 :completion 3
   :usd-in-1k 0.00015 :usd-out-1k 0.0006
   :tags (:coding :cheap :token-efficient :codemode))
  (:id "inception/mercury-2" :tier :lite :cost 1 :latency 2 :quality 5 :completion 5
   :usd-in-1k 0.00012 :usd-out-1k 0.00048
   :tags (:cheap :fast :reasoning :token-efficient :planning :software-dev))
  (:id "qwen/qwen3.5-flash-02-23" :tier :lite :cost 1 :latency 2 :quality 4 :completion 4
   :usd-in-1k 0.00010 :usd-out-1k 0.00040
   :tags (:cheap :fast :token-efficient :memory-ops :reasoning))
  (:id "deepseek/deepseek-chat-v3.1" :tier :lite :cost 2 :latency 3 :quality 4 :completion 3
   :usd-in-1k 0.00014 :usd-out-1k 0.00028
   :tags (:reasoning :cheap :token-efficient))
  (:id "minimax/minimax-m2.5" :tier :lite :cost 2 :latency 2 :quality 4 :completion 4
   :usd-in-1k 0.0002 :usd-out-1k 0.0011
   :tags (:cheap :fast :token-efficient :memory-ops))
  (:id "google/gemini-3.1-flash-lite-preview" :tier :micro :cost 1 :latency 1 :quality 3 :completion 3
   :usd-in-1k 0.000025 :usd-out-1k 0.0001
   :tags (:cheap :fast :token-efficient :memory-ops))
  (:id "moonshotai/kimi-k2.5" :tier :thinking :cost 6 :latency 5 :quality 7 :completion 7
   :usd-in-1k 0.002 :usd-out-1k 0.008
   :tags (:thinking :planning :software-dev))
  (:id "x-ai/grok-4.1-fast" :tier :fast-smart :cost 5 :latency 2 :quality 7 :completion 7
   :usd-in-1k 0.0002 :usd-out-1k 0.0005
   :features (:reasoning t :web-search t :x-search t :truth-seeking t)
   :native-tools
     (:reasoning (:enabled t :effort "high" :exclude t)
      :web-search (:plugin-id "web" :engine "native" :search-context-size "high")
      :x-search (:plugin-id "web" :engine "native" :search-context-size "high"))
   :tags (:truth-seeking :fast :strong :web-search :x-search :realtime))
  (:id "google/gemini-2.5-pro" :tier :pro :cost 7 :latency 5 :quality 8 :completion 8
   :usd-in-1k 0.0025 :usd-out-1k 0.015
   :tags (:reasoning :vision :codemode :software-dev))
  (:id "anthropic/claude-sonnet-4" :tier :pro :cost 8 :latency 5 :quality 9 :completion 9
   :usd-in-1k 0.003 :usd-out-1k 0.015
   :tags (:safety :reasoning :writing :codemode :software-dev))
  (:id "anthropic/claude-sonnet-4.6" :tier :pro :cost 8 :latency 4 :quality 9 :completion 9
   :usd-in-1k 0.003 :usd-out-1k 0.015
   :tags (:safety :reasoning :writing :codemode :software-dev))
  (:id "anthropic/claude-opus-4.6" :tier :frontier :cost 10 :latency 7 :quality 10 :completion 10
   :usd-in-1k 0.015 :usd-out-1k 0.075
   :tags (:frontier :reasoning :writing :codemode :software-dev))
  (:id "openai/gpt-5" :tier :pro :cost 9 :latency 6 :quality 10 :completion 10
   :usd-in-1k 0.005 :usd-out-1k 0.015
   :tags (:frontier :reasoning :codemode :software-dev))
  (:id "ber1-ai/qwen3.5-27b" :tier :free :cost 0 :latency 4 :quality 7 :completion 7
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :features (:reasoning t)
   :native-tools (:reasoning (:enabled t :effort "high" :exclude t))
   :tags (:free :reasoning :orchestration :software-dev :coding :planning :structured-output))
  (:id "ber1-ai/magistral-24b" :tier :free :cost 0 :latency 5 :quality 6 :completion 6
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :features (:reasoning t)
   :native-tools (:reasoning (:enabled t :effort "high" :exclude t))
   :tags (:free :reasoning :orchestration :planning :software-dev))
  (:id "ber1-ai/nanbeige-3b" :tier :free :cost 0 :latency 1 :quality 4 :completion 4
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:free :fast :execution :memory-ops :casual :structured-output)))
 :task-routing
 (:software-dev (:models ("anthropic/claude-opus-4.6" "anthropic/claude-sonnet-4.6"
                           "openai/gpt-5" "moonshotai/kimi-k2.5" "google/gemini-2.5-pro")
                  :cli-preference ("claude-code" "codex")
                  :openrouter-fallback t)
  :memory-ops (:models ("qwen/qwen3.5-flash-02-23" "inception/mercury-2"
                         "minimax/minimax-m2.5" "google/gemini-3.1-flash-lite-preview"
                         "amazon/nova-micro-v1" "ber1-ai/nanbeige-3b"))
  :truth-seeking (:models ("x-ai/grok-4.1-fast"
                           "google/gemini-2.5-pro"
                           "anthropic/claude-sonnet-4.6"
                           "openai/gpt-5")
                  :openrouter-fallback t)
  :casual (:models ("inception/mercury-2" "qwen/qwen3.5-flash-02-23"
                     "google/gemini-3.1-flash-lite-preview" "amazon/nova-lite-v1"
                     "qwen/qwen3-coder" "ber1-ai/nanbeige-3b"))
  :general (:models ("inception/mercury-2" "minimax/minimax-m2.5"
                      "qwen/qwen3.5-flash-02-23"
                      "ber1-ai/qwen3.5-27b" "ber1-ai/nanbeige-3b")))
 :evolution
 (:seed-models ()
  :active-provider "openrouter"
  :seed-provider-models
  (:openrouter ("inception/mercury-2"
                "qwen/qwen3.5-flash-02-23"
                "minimax/minimax-m2.5"
                "google/gemini-3.1-flash-lite-preview"
                "x-ai/grok-4.1-fast")
   :openai ("openai/gpt-5")
   :anthropic ("anthropic/claude-sonnet-4.6" "anthropic/claude-opus-4.6")
   :xai ("x-ai/grok-4.1-fast")
   :google-ai-studio ("google/gemini-3.1-flash-lite-preview" "google/gemini-2.5-pro")
   :google-vertex ("google/gemini-3.1-flash-lite-preview" "google/gemini-2.5-pro")
   :bedrock ("amazon/nova-micro-v1" "amazon/nova-lite-v1" "amazon/nova-pro-v1")
   :groq ("qwen/qwen3-coder")
   :alibaba ("qwen/qwen3-coder")
   :harmoniis ("ber1-ai/qwen3.5-27b" "ber1-ai/magistral-24b" "ber1-ai/nanbeige-3b"))
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
  :actor-stall-threshold 180
  :cli-cooloff-seconds 3600
  :cli-quota-patterns ("quota" "rate limit" "cooldown" "usage cap" "too many requests")
  :vitruvian-signal-min 0.62
  :context-summarizer-model "qwen/qwen3.5-flash-02-23"
  :context-summarizer-threshold-chars 12000
  :orchestrator-delegate-swarm t
  :orchestrator-model "inception/mercury-2"
  :orchestrator-enabled t)
 :routing-rules
 (:version 1
  :task-tier-hints
    ((:task :memory-ops :preferred-tier :eco)
     (:task :critical-reasoning :preferred-tier :premium)
     (:task :truth-seeking :preferred-tier :auto))
  :model-bans ()
  :model-boosts ()
  :cascade-config (:max-escalations 3 :confidence-threshold 0.7)))
