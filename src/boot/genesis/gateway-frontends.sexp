(:title "Gateway And Frontends"
 :description "Gateway is Harmonia's signal baseband processor and unified command dispatch point."
 :lisp-wrapper "src/ports/baseband.lisp"
 :rust-core "lib/core/gateway"

 :sections
  ((:name "Gateway Responsibilities"
    :items
     ("intercept and dispatch ALL /commands from ALL frontends (unified command dispatch)"
      "register/unregister frontend plugins"
      "parse and store frontend capabilities from config at registration time"
      "adapt frontend poll output into typed Baseband Channel Protocol envelopes"
      "send outbound payloads (with A2UI text fallback for non-capable frontends)"
      "expose frontend status, capabilities, and channel inventory"
      "provide one unified messaging boundary to the conductor"))

   (:name "Unified Command Dispatch"
    :source "lib/core/gateway/src/command_dispatch.rs"
    :description "The gateway is the single interception point for ALL /commands from ALL frontends."
    :tiers
     ((:name "Native" :description "Fully executed in Rust: /wallet, /identity, /help")
      (:name "Delegated" :description "Routed to Lisp-registered callback for runtime state: /status, /backends, /frontends, /tools, /chronicle, /metrics, /security, /feedback, /exit"))
    :security "Gateway enforces security labels before dispatch: Owner/Authenticated for read-restricted commands, TUI-only for /exit."
    :callback "CommandQueryFn registered by Lisp during init-baseband-port. Gateway calls it for delegated commands, frees the malloc'd result string."
    :exit-handling "When /exit is intercepted, gateway sets pending_exit flag. Lisp checks this after each poll and stops the run-loop.")

   (:name "Frontend Contract"
    :abi "harmonia_frontend_* C-ABI"
    :functions ("version" "healthcheck" "init" "poll" "send" "last_error" "shutdown" "free_string")
    :loading "Hot-loaded via gateway from config/baseband.sexp.")

   (:name "Frontend Capabilities"
    :description "Each frontend declares capabilities in config/baseband.sexp via :capabilities."
    :properties ("Static per-frontend — declared in config, not changeable at runtime."
                 "Attached to every channel envelope from that frontend as typed capability metadata."
                 "Queryable via gateway-frontend-status.")
    :principle "A2UI dispatch is generic: any frontend declaring :a2ui gets A2UI treatment. No hardcoded frontend-name checks.")

   (:name "Baseband Channel Envelope"
    :fields ("id" "version" "kind" "type-name" "channel" "peer" "body"
             "capabilities" "security" "audit" "transport" "attachments"))

   (:name "Poll Format"
    :formats
     ((:fields 2 :description "backward compatible: sub_channel TAB payload")
      (:fields 3 :description "with metadata: sub_channel TAB payload TAB metadata_sexp")
      (:fields 1 :description "no sub-channel: payload")))

   (:name "Auto-Load Policy"
    :function "register-configured-frontends"
    :modes ((:value t :description "always load")
            (:value nil :description "never load")
            (:value :if-vault-keys :description "load only if required vault symbols exist")))

   (:name "Inbound Signal Adaptation"
    :flow ("Gateway polls each registered frontend."
           "Command envelopes (/wallet, /status, etc.) are intercepted by command_dispatch — handled in Rust or delegated to Lisp callback — and filtered out."
           "Remaining envelopes (agent prompts) are adapted into Baseband Channel Protocol envelopes."
           "loop.lisp converts each envelope into a typed harmonia-signal struct."
           "The conductor renders a clean LLM summary from that struct."
           "Signals with high dissonance are attenuated in security-aware routing."
           "When a signal carries A2UI capability, the conductor injects the A2UI component catalog."))

   (:name "A2UI Component Catalog"
    :source "config/a2ui-catalog.sexp"
    :components 21
    :text-fallback "When the conductor sends an A2UI payload to a non-A2UI frontend, it extracts plain text from the component data.")

   (:name "Push Integration"
    :location "lib/frontends/push"
    :purpose "Utility library consumed by mqtt-client for offline device push notifications via HTTP webhook.")

   (:name "Outbound Flow"
    :queue "*gateway-outbound-queue*"
    :flush-phase ":gateway-flush"
    :a2ui-degradation "A2UI payloads sent to text-only frontends are automatically degraded to plain text.")

   (:name "Signal Security"
    :layers
     ((:name "Dissonance scoring" :description "Gateway baseband scans signal payloads for injection patterns and assigns a dissonance score (0.0-1.0).")
      (:name "Typed dispatch" :description "Loop.lisp creates harmonia-signal structs from typed envelopes with :taint :external.")
      (:name "Policy gate" :description "LLM-proposed tool commands from external signals must pass %policy-gate. Privileged operations denied for tainted origins.")
      (:name "MQTT fingerprint validation" :description "MQTT frontend validates agent_fp against vault-stored expected fingerprint.")
      (:name "Tailnet HMAC" :description "Mesh messages authenticated with HMAC-SHA256 and protected against replay (5-minute window).")))))
