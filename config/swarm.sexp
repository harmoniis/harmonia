(:subagent-count 1
 :tmux (:max-concurrent 5
        :poll-interval-ms 3000
        :session-prefix "harmonia-"
        :default-cli "claude-code"
        :default-autonomy :max
        :spawn-timeout-s 30
        :idle-timeout-s 300
        :pane-width 220
        :pane-height 50
        :sanitize-env ("CLAUDECODE" "CLAUDE_CODE_ENTRYPOINT")
        :cli-profiles
          ((:type "claude-code"
            :launch "claude"
            :auto-flags ("--dangerously-skip-permissions")
            :noninteractive-flag "-p"
            :detection-window 15)
           (:type "codex"
            :launch "codex"
            :auto-flags ("--full-auto")
            :noninteractive-cmd "exec"
            :detection-window 12)))
 :openrouter (:max-parallel 3
              :verify-results t
              :connect-timeout-s 10
              :max-time-s 45)
 :self-rewrite (:primary-cli "claude-code"
                :fallback-cli "codex"
                :model-weights (:quality 0.40 :completion 0.35 :correctness 0.15
                                :speed 0.05 :price 0.05)
                :validation-gates (:compile t :test t :ffi-compat t
                                   :score-min-ratio 0.95 :size-max-ratio 1.5))
 ;; Prompt templates consolidated into config/prompts.sexp
 :software-dev
   (:phases
     ((:name :planning
       :models ("anthropic/claude-opus-4.6" "openai/gpt-5")
       :weights (:quality 0.50 :correctness 0.20 :speed 0.10 :price 0.10 :completion 0.10))
      (:name :implementation
       :models ("anthropic/claude-sonnet-4.6" "moonshotai/kimi-k2.5")
       :weights (:quality 0.25 :correctness 0.20 :speed 0.20 :price 0.10 :completion 0.25))
      (:name :review
       :models ("anthropic/claude-opus-4.6" "google/gemini-2.5-pro")
       :weights (:quality 0.30 :correctness 0.25 :speed 0.10 :price 0.10 :completion 0.25)))
    :cli-preference ("claude-code" "codex")
    :openrouter-fallback t
    :fallback-chain ("minimax/minimax-m2.5" "moonshotai/kimi-k2.5"
                     "anthropic/claude-opus-4.6" "anthropic/claude-sonnet-4.6")))
