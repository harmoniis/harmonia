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
  (:id "qwen/qwen3-coder:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:coding :cheap :token-efficient :codemode))
  (:id "deepseek/deepseek-chat-v3.1:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:reasoning :cheap :token-efficient))
  (:id "minimax/minimax-m2.5" :tier :lite :cost 2 :latency 2 :quality 4 :completion 4
   :usd-in-1k 0.0002 :usd-out-1k 0.0011
   :tags (:cheap :fast :token-efficient :memory-ops))
  (:id "google/gemini-3.1-flash-lite-preview" :tier :micro :cost 1 :latency 1 :quality 3 :completion 3
   :usd-in-1k 0.0 :usd-out-1k 0.0
   :tags (:cheap :fast :token-efficient :memory-ops))
  (:id "moonshotai/kimi-k2.5" :tier :thinking :cost 6 :latency 5 :quality 7 :completion 7
   :usd-in-1k 0.002 :usd-out-1k 0.008
   :tags (:thinking :planning :software-dev))
  (:id "x-ai/grok-4-fast:online" :tier :fast-smart :cost 5 :latency 2 :quality 7 :completion 7
   :usd-in-1k 0.003 :usd-out-1k 0.015
   :features (:reasoning t :web-search t :x-search t)
   :tags (:planner :fast :strong :codemode))
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
   :tags (:frontier :reasoning :codemode :software-dev)))
 :task-routing
 (:software-dev (:models ("anthropic/claude-opus-4.6" "anthropic/claude-sonnet-4.6"
                           "openai/gpt-5" "moonshotai/kimi-k2.5" "google/gemini-2.5-pro")
                  :cli-preference ("claude-code" "codex")
                  :openrouter-fallback t)
  :memory-ops (:models ("google/gemini-3.1-flash-lite-preview" "minimax/minimax-m2.5"
                         "deepseek/deepseek-chat-v3.1:free" "amazon/nova-micro-v1"))
  :casual (:models ("amazon/nova-lite-v1" "qwen/qwen3-coder:free"
                     "deepseek/deepseek-chat-v3.1:free"))))
