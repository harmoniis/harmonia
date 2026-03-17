(:frontends
  ((:name "tui"
    :so-path "target/release/libharmonia_tui.so"
    :security-label :owner
    :auto-load t
    :vault-keys nil)
   (:name "mqtt"
    :so-path "target/release/libharmonia_mqtt_client.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:mqtt-broker-url :mqtt-cert)
    :capabilities (:a2ui "1.0" :push "t")
    :push-webhook-url ""
    :push-webhook-token ""
    :push-webhook-timeout-ms 5000)
   (:name "http2"
    :so-path "target/release/libharmonia_http2_mtls.so"
    :security-label :authenticated
    :auto-load :if-ready
    :vault-keys nil
    :config-keys (("http2-frontend" "bind")
                  ("http2-frontend" "ca-cert")
                  ("http2-frontend" "server-cert")
                  ("http2-frontend" "server-key")
                  ("http2-frontend" "trusted-client-fingerprints-json")))
   (:name "imessage"
    :so-path "target/release/libharmonia_imessage.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:bluebubbles-password)
    :platforms (:macos))
   (:name "whatsapp"
    :so-path "target/release/libharmonia_whatsapp.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:whatsapp-session))
   (:name "telegram"
    :so-path "target/release/libharmonia_telegram.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:telegram-bot-token))
   (:name "slack"
    :so-path "target/release/libharmonia_slack.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:slack-app-token :slack-bot-token))
   (:name "discord"
    :so-path "target/release/libharmonia_discord.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:discord-bot-token))
   (:name "signal"
    :so-path "target/release/libharmonia_signal.so"
    :security-label :authenticated
    :auto-load :if-ready
    :vault-keys nil
    :config-keys (("signal-frontend" "account")))
   (:name "tailscale"
    :so-path "target/release/libharmonia_tailscale_frontend.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:tailscale-auth-key))
   (:name "email"
    :so-path "target/release/libharmonia_email_client.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:email-imap-password :email-password))
   (:name "mattermost"
    :so-path "target/release/libharmonia_mattermost.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:mattermost-bot-token))
   (:name "nostr"
    :so-path "target/release/libharmonia_nostr.so"
    :security-label :untrusted
    :auto-load :if-vault-keys
    :vault-keys (:nostr-private-key)))
 :tools
  ((:name "browser"
    :so-path "target/release/libharmonia_browser.dylib"
    :security-label :authenticated
    :auto-load t
    :vault-keys nil)
   (:name "search-exa"
    :so-path "target/release/libharmonia_search_exa.dylib"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:exa-api-key))
   (:name "search-brave"
    :so-path "target/release/libharmonia_search_brave.dylib"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:brave-api-key))
   (:name "zoom"
    :so-path "target/release/libharmonia_zoom.dylib"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:zoom-email :zoom-password)))
 :mesh
  (:transport :tailscale-only
   :discovery-interval-s 30
   :heartbeat-interval-s 10))
