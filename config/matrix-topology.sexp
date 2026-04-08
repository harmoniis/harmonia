(:nodes
  ;; Core — infrastructure
  (("orchestrator" . "core")
   ("memory" . "core")
   ("vault" . "core")
   ("harmonic-matrix" . "core")
   ("gateway" . "core")
   ("parallel-agents" . "core")

   ;; Backends — provider nodes
   ("provider-router" . "backend")
   ("voice-router" . "backend")

   ;; Tools — capability channels
   ("browser" . "tool")
   ("search-exa" . "tool")
   ("search-brave" . "tool")
   ("zoom" . "tool"))

 :edges
  ;; Orchestrator → core infrastructure
  (("orchestrator" "vault" 1.20 0.70)
   ("orchestrator" "harmonic-matrix" 1.20 0.60)
   ("orchestrator" "memory" 1.25 0.10)
   ("orchestrator" "gateway" 1.20 0.15)

   ;; Orchestrator → backends
   ("orchestrator" "provider-router" 1.15 0.35)
   ("orchestrator" "voice-router" 1.10 0.30)
   ("orchestrator" "parallel-agents" 1.10 0.30)

   ;; Orchestrator → tools
   ("orchestrator" "browser" 1.10 0.25)
   ("orchestrator" "search-exa" 1.20 0.20)
   ("orchestrator" "search-brave" 1.10 0.20)
   ("orchestrator" "zoom" 1.00 0.25)

   ;; Tool → Browser (service tools that use browser as foundation)
   ("zoom" "browser" 1.15 0.20)

   ;; Parallel agents → providers and tools
   ("parallel-agents" "provider-router" 1.05 0.25)
   ("parallel-agents" "search-exa" 1.10 0.20)
   ("parallel-agents" "search-brave" 1.05 0.20)

   ;; Everything → memory (telemetry sink)
   ("provider-router" "memory" 0.95 0.05)
   ("voice-router" "memory" 0.90 0.05)
   ("browser" "memory" 0.90 0.05)
   ("search-exa" "memory" 1.00 0.05)
   ("search-brave" "memory" 0.95 0.05)
   ("zoom" "memory" 0.90 0.05)
   ("vault" "memory" 0.95 0.05)
   ("harmonic-matrix" "memory" 0.95 0.05)
   ("parallel-agents" "memory" 0.95 0.05)
   ("gateway" "memory" 0.95 0.05))

 :tools
  (("browser" . t)
   ("search-exa" . t)
   ("search-brave" . t)
   ("zoom" . t)))
