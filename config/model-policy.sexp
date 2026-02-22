(:weights (:completion 0.34 :correctness 0.20 :speed 0.14 :price 0.12
           :token-efficiency 0.10 :orchestration-efficiency 0.10)
 :profiles
 ((:id "amazon/nova-micro-v1" :tier :micro :cost 1 :latency 1 :quality 2 :completion 2
   :tags (:cheap :fast :routing :token-efficient :tool-use))
  (:id "amazon/nova-lite-v1" :tier :lite :cost 2 :latency 2 :quality 3 :completion 3
   :tags (:cheap :vision :ocr :token-efficient))
  (:id "amazon/nova-pro-v1" :tier :pro :cost 4 :latency 3 :quality 5 :completion 5
   :tags (:balanced :reasoning :tooling :tool-use))
  (:id "qwen/qwen3-coder:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :tags (:coding :cheap :token-efficient :codemode))
  (:id "deepseek/deepseek-chat-v3.1:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :tags (:reasoning :cheap :token-efficient))
  (:id "moonshotai/kimi-k2.5" :tier :thinking :cost 6 :latency 5 :quality 7 :completion 7
   :tags (:thinking :planning))
  (:id "x-ai/grok-4-fast:online" :tier :fast-smart :cost 5 :latency 2 :quality 7 :completion 7
   :tags (:planner :fast :strong :codemode))
  (:id "google/gemini-2.5-pro" :tier :pro :cost 7 :latency 5 :quality 8 :completion 8
   :tags (:reasoning :vision :codemode))
  (:id "anthropic/claude-sonnet-4" :tier :pro :cost 8 :latency 5 :quality 9 :completion 9
   :tags (:safety :reasoning :writing :codemode))
  (:id "openai/gpt-5" :tier :pro :cost 9 :latency 6 :quality 10 :completion 10
   :tags (:frontier :reasoning :codemode))))
