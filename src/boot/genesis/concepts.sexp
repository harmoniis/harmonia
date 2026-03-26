(:title "Core Concepts"
 :purpose "Conceptual model that should remain stable across implementation changes."

 :sections
  ((:name "Harmony As Operational Discipline"
    :content "Harmony in Harmonia is not aesthetic language only. It is operationalized as: high completion with low failure, low noise in routing and memory, and composable structures that can be validated and evolved.")

   (:name "Compression As Intelligence Pressure"
    :content "The system prefers compressed representations that preserve utility."
    :examples ("daily interactions compressed into reusable skills"
               "codemode pipelines collapsing many relay turns into one deterministic tool chain"
               "policy in data files instead of deeply nested hardcoded branches"))

   (:name "Attractor-Seeking Runtime"
    :content "Harmonic planning uses attractor-inspired dynamics (logistic, lambdoma, lorenz) to steer rewrite timing. Goal: avoid both chaotic rewrites and static stagnation.")

   (:name "Genomic vs Epigenetic Layers"
    :content "Genomic layer: source and configuration structure. Epigenetic layer: runtime weights, scores, and mutable policy state. Healthy evolution keeps these layers synchronized without collapsing them.")

   (:name "Four-Pillar Capability Model"
    :content "Rust capability surface is intentionally partitioned: core, backends, tools, frontends. This keeps expansion predictable and boundaries clear.")

   (:name "Boundary-First Safety"
    :content "Three boundaries are central: vault boundary for secrets, matrix boundary for route permissions, gateway boundary for channel ingress/egress. Any evolution that weakens one boundary increases systemic risk.")

   (:name "Capabilities Over Names"
    :content "Frontend behavior is driven by declared capabilities, not identity checks. A frontend declares what it can do (:a2ui, :push) in its baseband config. The conductor inspects signal capabilities, never frontend names. This keeps the architecture open for any future frontend.")

   (:name "Signal Enrichment"
    :content "Gateway signals carry two enrichment layers beyond payload: capabilities (static, from config) for what the frontend can do, and metadata (dynamic, per-message) for what the specific device/session provides. This separation keeps the agent informed without coupling signal processing to specific frontend implementations.")

   (:name "Security Kernel"
    :content "The security kernel is a deterministic, non-bypassable layer that protects privileged operations."
    :components
     ((:name "Typed signals" :description "External data enters as harmonia-signal structs with security labels and taint tags, never as raw executable strings.")
      (:name "Policy gate" :description "Binary allow/deny gate for privileged operations. Checks taint chain and security label — not harmonic scores.")
      (:name "Taint propagation" :description "*current-originating-signal* tracks the signal that initiated each reasoning chain.")
      (:name "Invariant guards" :description "Hardcoded safety limits that cannot be weakened by configuration or admin intent."))
    :principle "LLM output is a proposal, not a command. For non-privileged operations, proposals flow through harmonic routing. For privileged operations, proposals must pass the deterministic policy gate.")

   (:name "Adaptive Security Shell"
    :content "Complementing the hard security kernel, the adaptive shell provides defense-in-depth."
    :components
     ((:name "Dissonance scoring" :description "Injection pattern detection at gateway ingestion, producing a 0.0-1.0 dissonance score per signal.")
      (:name "Security-aware routing" :description "Harmonic matrix attenuates signals with high dissonance or low security weight.")
      (:name "Security posture tracking" :description "Autonomous monitoring of injection rates per frontend, with auto-adjustment of noise floors.")
      (:name "Boundary wrapping" :description "External data in prompts, memory recalls, and tool results is wrapped with security markers to resist prompt injection."))
    :summary "The kernel stops exploits structurally. The shell detects and attenuates anomalies adaptively.")

   (:name "Chronicle As Institutional Memory"
    :content "The chronicle knowledge base (lib/core/chronicle) is the agent's durable, queryable memory of its own evolution."
    :features
     ("Harmonic snapshots decompose every cycle's vitruvian scores, chaos dynamics, and attractor state into SQL-queryable rows."
      "Concept graphs from memory-map-sexp are decomposed into relational graph_nodes and graph_edges tables, enabling recursive CTE traversal and bridge detection through standard SQL."
      "Delegation decisions capture which model was chosen, why, at what cost, and whether it succeeded."
      "Pressure-aware GC preserves high-signal data while thinning noise.")
    :insight "The agent does not just log — it builds a queryable knowledge graph that it can reason over to inform its next evolution.")

   (:name "Memory As Energy In Fields"
    :content "Memory recall is field relaxation on the concept graph, not database search. The graph Laplacian propagates activation from query concepts to resonant memory nodes. Attractor basins (Thomas, Aizawa, Halvorsen) partition memory into dynamical regimes with hysteresis barriers. Spectral eigenmodes (Chladni patterns) give frequency-selective recall."
    :components
     ((:name "Field propagation" :description "Solving L·φ = b on the concept graph via conjugate gradient — lightning finds the path.")
      (:name "Attractor basins" :description "Thomas (6 domains), Aizawa (depth), Halvorsen (bridging) — each basin geometry serves a memory dimension.")
      (:name "Hysteresis" :description "Basin switching requires sustained coercive energy. Weak signals don't hijack context.")
      (:name "Chladni modes" :description "Spectral eigenvectors of the Laplacian — standing wave patterns for frequency-selective recall.")
      (:name "Warm-start" :description "Basin state persists in Chronicle across restarts. The system remembers where it was."))
    :principle "Memory is resonance, not matching. The system vibrates at the frequency of the incoming signal, and memories that resonate naturally activate.")

   (:name "Guardian Healer — LLM-Guarded Self-Healing"
    :content "A guardian LLM diagnoses failures and proposes SAFE actions from a whitelist. The healer can never execute arbitrary code or bypass the policy gate — it can only restart components, switch models, skip features, reload config, and report to operators."
    :levels
     ((:level 0 :name "Retry" :description "Transient errors: IPC timeout, temporary backend unavailability.")
      (:level 1 :name "Fallback" :description "Use simpler method: field recall falls back to substring, expensive model falls back to cheaper one.")
      (:level 2 :name "Pattern" :description "Detect repeating errors, classify root cause from error ring history.")
      (:level 3 :name "Guardian" :description "LLM diagnoses from error context, proposes one safe action from whitelist.")
      (:level 4 :name "Restart" :description "Restart failed component via IPC reset. Complements Phoenix process-level restarts.")
      (:level 5 :name "Report" :description "Honest, helpful message to user. Never 'internal error'."))
    :guardian-principle "The healer operates with :internal taint and a whitelist of safe actions. It cannot mutate vault, change policy, rewrite security, or execute code. All recovery events are recorded to Chronicle for learning."
    :principle "Resilience is not enough. A living system must heal, not just survive.")

   (:name "Evolution With Rollback"
    :content "Every meaningful rewrite path must preserve rollback viability. Improvement without rollback is treated as unsafe mutation, not evolution.")))
