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
   (:name "imessage"
    :so-path "target/release/libharmonia_imessage.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:bluebubbles-password))
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
    :auto-load t
    :vault-keys nil)
   (:name "tailscale"
    :so-path "target/release/libharmonia_tailscale_frontend.so"
    :security-label :authenticated
    :auto-load :if-vault-keys
    :vault-keys (:tailscale-auth-key)))
 :mesh
  (:transport :tailscale-only
   :discovery-interval-s 30
   :heartbeat-interval-s 10))
