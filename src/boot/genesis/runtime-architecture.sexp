(:title "Runtime Architecture"

 :sections
  ((:name "Runtime Topology"
    :content "Harmonia runtime is Lisp-first orchestration with Rust execution ports. Lisp coordinates prompts, memory, model selection, routing, and loop control. Rust crates provide external capabilities through C-ABI and CFFI.")

   (:name "Boot Flow"
    :entry-point "src/core/boot.lisp"
    :sequence
     ("Validate environment safety (%enforce-genesis-safety)."
      "Initialize runtime state (make-runtime-state)."
      "Validate DNA (dna-valid-p)."
      "Register tools from config/tools.sexp."
      "Seed soul memory from DNA."
      "Initialize ports in strict order: vault, store, harmony-policy, model-policy, router, lineage, matrix, tool-runtime, baseband frontends, swarm, evolution, chronicle, signalograd."
      "Restore the evolution-matched signalograd checkpoint if present."))

   (:name "Deterministic Tick Model"
    :source "src/core/loop.lisp"
    :tick-actions
     ("gateway-poll" "tailnet-poll" "actor-supervisor" "process-prompt"
      "actor-deliver" "memory-heartbeat" "harmonic-step"
      "chronicle-flush" "gateway-flush" "tailnet-flush")
    :error-handling "Each action is wrapped in %supervised-action (Erlang-style). Errors are caught, recorded to the error ring buffer, and the tick continues. The loop never crashes."
    :adaptive-cooldown "After 10 consecutive error ticks, sleep interval increases 5x to prevent error storms.")

   (:name "Gateway Signal Processing"
    :content "During :gateway-poll, the gateway first intercepts all /commands via unified command dispatch (command_dispatch.rs). Native commands (/wallet, /identity, /help) are handled in Rust; delegated commands (/status, /backends, etc.) are routed to Lisp via registered callback. Command responses are sent back to the originating frontend. Only non-command envelopes pass through to the Lisp orchestrator as Baseband Channel Protocol envelopes with typed :channel, :peer, :body, :capabilities, :security, :audit, and :transport sections. Signals with high dissonance are attenuated in security-aware routing.")

   (:name "Orchestration Flow"
    :source "src/orchestrator/conductor.lisp"
    :dispatch-types
     ((:type "harmonia-signal" :handler "orchestrate-signal" :description "Binds *current-originating-signal*, boundary-wraps payload, sends to LLM. Tool commands are proposed actions that must pass %policy-gate.")
      (:type "string" :handler "orchestrate-prompt" :description "Internal/TUI prompt. *current-originating-signal* is nil (owner trust). May contain direct tool commands."))
    :prompt-assembly
     ("DNA constitution"
      "bootstrap memory block (boundary-wrapped recalls)"
      "semantic recall block (boundary-wrapped)"
      "A2UI component catalog (if signal has A2UI capability)")
    :security "LLM-proposed tool commands from external signals must pass %policy-gate. Privileged operations are denied for tainted origins.")

   (:name "Harmonic State Machine"
    :source "src/core/harmonic-machine.lisp"
    :phases ("observe" "evaluate-global" "evaluate-local" "logistic-balance"
             "lambdoma-project" "attractor-sync" "rewrite-plan"
             "security-audit" "stabilize")
    :signalograd-coupling
     ("chronicle records the finished cycle"
      "Lisp sends signalograd feedback for the previous applied projection"
      "Lisp sends a new telemetry observation"
      "Rust advances the chaotic reservoir / attractor memory state"
      "Rust posts a bounded proposal through the unified actor mailbox"
      "Lisp applies that proposal only on the next cycle after policy clamps")
    :property "This makes the adaptive layer causal, auditable, and actor-model aligned.")

   (:name "Error Discipline And Self-Repair"
    :content "Runtime errors are classified (compiler, backend, evolution) and recorded via src/core/conditions.lisp."
    :supervision "The supervision layer (%supervised-action) catches all serious-condition errors, records them to a 64-entry circular error ring (*error-ring*), and increments counters."
    :self-knowledge-source "src/core/introspection.lisp"
    :self-knowledge-features
     ("Platform and path introspection for autonomous debugging."
      "introspect-runtime — full diagnostic snapshot."
      "introspect-recent-errors — last N errors with context."
      "introspect-libs — all loaded cdylibs with crash counts."
      "%cargo-build-component — self-compilation of individual crates."
      "%hot-reload-frontend — rebuild, copy, and re-register a frontend cdylib."))

   (:name "Security Architecture"
    :layers
     ((:name "Security Kernel (deterministic)"
       :features
        ("Typed signal dispatch separates external signals from internal prompts."
         "%policy-gate enforces binary allow/deny on 14 privileged operations."
         "*current-originating-signal* propagates taint through the reasoning chain."
         "Safe parsers eliminate read-from-string ACE vectors."
         "Invariant guards enforce non-configurable safety limits."))
      (:name "Adaptive Shell (harmonic)"
       :features
        ("Dissonance scoring at gateway ingestion."
         "Security-aware routing via route_allowed_with_context."
         ":security-audit phase tracks posture (:nominal/:elevated/:alert)."))
      (:name "Transport Security"
       :features
        ("Tailnet HMAC-SHA256 authentication with replay protection."
         "MQTT fingerprint validation against vault-stored expected values."
         "Wallet-rooted vault encryption at rest (AES-256-GCM) with audit logging.")))
    :config "config/harmony-policy.sexp :security section")))
