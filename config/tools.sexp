(
  ;; Core — essential infrastructure
  ("phoenix" . "lib/core/phoenix")
  ("ouroboros" . "lib/core/ouroboros")
  ("vault" . "lib/core/vault")
  ("memory" . "lib/core/memory")
  ("http" . "lib/backends/http")
  ("s3" . "lib/backends/storage/s3")
  ("git-ops" . "lib/core/git-ops")
  ("rust-forge" . "lib/core/rust-forge")
  ("cron-scheduler" . "lib/core/cron-scheduler")
  ("push" . "lib/frontends/push")
  ("recovery" . "lib/core/recovery")
  ("fs" . "lib/core/fs")
  ("parallel-agents" . "lib/core/parallel-agents")
  ("harmonic-matrix" . "lib/core/harmonic-matrix")
  ("config-store" . "lib/core/config-store")
  ("gateway" . "lib/core/gateway")
  ("tailnet" . "lib/core/tailnet")
  ("chronicle" . "lib/core/chronicle")

  ;; Backends — LLM providers
  ("openrouter-backend" . "lib/backends/llms/openrouter")

  ;; Backends — Voice providers
  ("voice-router" . "lib/backends/voice/voice-router")

  ;; Tools — utility plugins
  ("browser" . "lib/tools/browser")
  ("search-exa" . "lib/tools/search-exa")
  ("search-brave" . "lib/tools/search-brave")
  ("hfetch" . "lib/tools/hfetch")
  ("zoom" . "lib/tools/zoom")

  ;; Frontends — hot-pluggable channels (loaded via gateway, not directly)
  ;; Listed here for reference; actual loading is via baseband.sexp
  ("mqtt-client" . "lib/frontends/mqtt-client")
  ("http2-mtls" . "lib/frontends/http2-mtls")
  ("tui" . "lib/frontends/tui")
  ("imessage" . "lib/frontends/imessage")
  ("whatsapp" . "lib/frontends/whatsapp")
  ("telegram" . "lib/frontends/telegram")
  ("slack" . "lib/frontends/slack")
  ("discord" . "lib/frontends/discord")
  ("signal" . "lib/frontends/signal")
  ("tailscale" . "lib/frontends/tailscale")
  ("mattermost" . "lib/frontends/mattermost")
  ("nostr" . "lib/frontends/nostr")
  ("email-client" . "lib/frontends/email-client")
)
