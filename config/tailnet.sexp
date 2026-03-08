(:tailnet
  (:auth-key-vault-symbol :tailscale-auth-key
   :hostname-prefix "harmonia-"
   :allowed-tags ("tag:harmonia-node")
   :discovery-interval-s 30
   :heartbeat-interval-s 10
   :message-port 7483))
