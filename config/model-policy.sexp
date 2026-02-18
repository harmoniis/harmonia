(:weights (:completion 0.46 :correctness 0.24 :speed 0.16 :price 0.14)
 :profiles
 ((:id "amazon/nova-micro-v1" :tier :micro :cost 1 :latency 1 :quality 2 :completion 2
   :tags (:cheap :fast :routing))
  (:id "amazon/nova-lite-v1" :tier :lite :cost 2 :latency 2 :quality 3 :completion 3
   :tags (:cheap :vision :ocr))
  (:id "amazon/nova-pro-v1" :tier :pro :cost 4 :latency 3 :quality 5 :completion 5
   :tags (:balanced :reasoning :tooling))
  (:id "qwen/qwen3-coder:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :tags (:coding :cheap))
  (:id "deepseek/deepseek-chat-v3.1:free" :tier :free :cost 1 :latency 3 :quality 4 :completion 3
   :tags (:reasoning :cheap))
  (:id "moonshotai/kimi-k2.5" :tier :thinking :cost 6 :latency 5 :quality 7 :completion 7
   :tags (:thinking :planning))
  (:id "x-ai/grok-4-fast:online" :tier :fast-smart :cost 5 :latency 2 :quality 7 :completion 7
   :tags (:planner :fast :strong))
  (:id "google/gemini-2.5-pro" :tier :pro :cost 7 :latency 5 :quality 8 :completion 8
   :tags (:reasoning :vision))
  (:id "anthropic/claude-sonnet-4" :tier :pro :cost 8 :latency 5 :quality 9 :completion 9
   :tags (:safety :reasoning :writing))
  (:id "openai/gpt-5" :tier :pro :cost 9 :latency 6 :quality 10 :completion 10
   :tags (:frontier :reasoning))))
