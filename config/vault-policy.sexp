;;; vault-policy.sexp — Component-to-secret pattern mappings.
;;;
;;; Declarative policy: which components can read which vault symbols.
;;; Loaded at boot by harmonia-vault. Pattern syntax:
;;;   "exact-name"   — exact match
;;;   "*prefix"      — prefix match
;;;   "suffix*"      — suffix match
;;;   "*"            — match all
;;;
;;; To override at runtime: set HARMONIA_VAULT_COMPONENT_POLICY env var.

(:component-patterns
  ;; LLM backends
  ("openrouter-backend"       ("openrouter" "openrouter-api-key"))
  ("openai-backend"           ("openai" "openai-api-key"))
  ("anthropic-backend"        ("anthropic" "anthropic-api-key"))
  ("xai-backend"              ("xai" "xai-api-key" "x-ai-api-key"))
  ("google-ai-studio-backend" ("google-ai-studio-api-key" "gemini-api-key" "google-api-key"))
  ("google-vertex-backend"    ("google-vertex-access-token" "vertex-access-token"
                               "google-vertex-project-id" "vertex-project-id"
                               "google-vertex-location" "vertex-location"))
  ("amazon-bedrock-backend"   ("aws-access-key-id" "aws-secret-access-key"
                               "aws-session-token" "aws-region"))
  ("groq-backend"             ("groq" "groq-api-key"))
  ("alibaba-backend"          ("alibaba" "alibaba-api-key" "dashscope-api-key"))
  ("harmoniis-backend"        ("harmoniis" "harmoniis-api-key" "harmoniis-router-api-key"))
  ;; Tools
  ("search-exa-tool"          ("exa-api-key"))
  ("search-brave-tool"        ("brave-api-key"))
  ;; Voice
  ("whisper-backend"          ("groq-api-key" "groq" "openai-api-key" "openai"))
  ("elevenlabs-backend"       ("elevenlabs-api-key" "elevenlabs"))
  ;; Frontends
  ("email-frontend"           ("email-imap-password" "email-password"
                               "email-smtp-password" "email-api-key"))
  ("mattermost-frontend"      ("mattermost-bot-token" "mattermost-token"))
  ("nostr-frontend"           ("nostr-private-key" "nostr-nsec"))
  ("telegram-frontend"        ("telegram-bot-token" "telegram-bot-api-token"))
  ("slack-frontend"           ("slack-bot-token" "slack-app-token"
                               "slack-bot-token-v2" "slack-app-level-token"))
  ("discord-frontend"         ("discord-bot-token" "discord-token"))
  ("signal-frontend"          ("signal-auth-token" "signal-auth-token-v2"
                               "signal-account" "signal-rpc-url" "signal-bridge-url"))
  ("whatsapp-frontend"        ("whatsapp-session" "whatsapp-api-key" "whatsapp-bridge-url"))
  ("imessage-frontend"        ("bluebubbles-password" "imessage-password"
                               "bluebubbles-server-url" "imessage-server-url"))
  ("tailscale-frontend"       ("tailscale-auth-key"))
  ("mqtt-frontend"            ("mqtt-agent-fp" "mqtt-tls-master-seed"
                               "mqtt-tls-client-cert-pem" "mqtt-tls-client-key-pem"
                               "mqtt-tls-client-cert-path" "mqtt-tls-client-key-path"
                               "mqtt-broker-url"))
  ;; System
  ("admin-intent"             ("*pubkey"))
  ("parallel-agents-core"     ("openrouter" "openrouter-api-key"
                               "exa-api-key" "brave-api-key"))
  ("observability"            ("langsmith-api-key")))
