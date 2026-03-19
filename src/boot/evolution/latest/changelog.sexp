;; changelog.sexp — Append-only evolution ledger (newest first for efficient prepend)

((:version 10 :date "2026-03-11"
  :target "Unified command dispatch — gateway as single interception point for all /commands from all frontends."
  :motivation "All system commands must flow through one dispatch point regardless of frontend origin. Lisp is the agent brain (reasoning, orchestration); typed operations belong in Rust."
  :law "Single-responsibility dispatch; boundary-first safety; Lisp orchestrates, Rust executes."
  :changes ("Gateway command_dispatch.rs: single interception point for ALL /commands from ALL frontends (TUI, MQTT, Telegram, Tailscale, paired nodes)"
            "Two-tier command handling: native Rust handlers (/wallet, /identity, /help) and Lisp-delegated via callback (/status, /backends, /frontends, /tools, /chronicle, /metrics, /security, /feedback, /exit)"
            "CommandQueryFn callback: Lisp registers a C callback with the gateway for delegated commands needing runtime state"
            "Gateway enforces security (Owner/Authenticated for read-restricted, TUI-only for /exit) before dispatching"
            "pending_exit flag: gateway sets when /exit intercepted, Lisp checks after each poll to stop run-loop"
            "All crate Cargo.toml files unified to [cdylib, rlib] for dual loading (subsequently: cdylib removed, FFI replaced by Unix domain socket IPC with harmonia-runtime)"
            "system-commands.lisp refactored: %gateway-dispatch-command as callback entry point, security checks removed (gateway enforces)")
  :risk "Lisp command callback must be registered before any frontend sends commands. Gateway init happens before frontend registration."
  :rollback "Remove command interception from poll_baseband; remove callback registration from init-baseband-port; restore %maybe-dispatch-system-command as primary handler.")

 (:version 9 :date "2026-03-10"
  :target "Provider-scoped seed evolution, swarm-first orchestration, and operator-visible seed setup controls."
  :motivation "Avoid provider bias defaults, start from low-cost/high-throughput seed models, keep orchestrator focused on planning/delegation."
  :law "Harmonic optimization via explicit price/speed/success/reasoning/vitruvian weighting with auditable state."
  :changes ("Seed policy source of truth: provider-aware seed resolution chain"
            "OpenRouter default seeds: mercury-2, qwen3.5-flash, minimax-m2.5, gemini-3.1-flash-lite"
            "Seed evolution scoring with weighted dimensions and sample threshold gating"
            "CLI-first routing with per-invocation timeout and quota/cooloff detection"
            "Orchestrator delegates non-tool LLM work through swarm (parallel-solve)"
            "Optional large-context summarization before delegation"
            "Structured swarm outcome parsing with chronicle telemetry"
            "harmonia setup --seeds for seed-policy-only updates")
  :risk "Swarm-first delegation increases dependence on parallel-agents route availability."
  :rollback "Re-enable direct conductor backend completion path, remove seeds-only setup branch.")

 (:version 8 :date "2026-03-10"
  :target "Chronicle — Graph-native knowledge base and time-series observability."
  :motivation "The agent must durably record and reason over its own evolution via SQL-queryable data."
  :law "Compression as intelligence pressure; attractor-seeking runtime; know thyself."
  :changes ("Chronicle crate: SQLite WAL-mode knowledge base, 9 tables"
            "query_sexp(sql): arbitrary SQL returning s-expression results"
            "Concept graph decomposition into relational tables with CTE traversal"
            "Pressure-aware GC: soft/hard/critical tiers preserving inflection points"
            "Harmony trajectory: permanently downsampled 5-minute buckets, never pruned"
            "Chronicle port: 40+ CFFI bindings, query API returning parsed s-expressions"
            "Harmonic machine :stabilize phase records full snapshot + concept graph"
            "Memory/conductor/phoenix/ouroboros integration"
            "A2UI dashboard: 8-panel Composite")
  :new-crates ("lib/core/chronicle")
  :risk "Chronicle is append-only. All integration points use ignore-errors."
  :rollback "Remove init-chronicle-port from boot.lisp; remove chronicle calls from harmonic-machine, compression, conductor.")

 (:version 7 :date "2026-03-10"
  :target "Erlang-style fault tolerance, runtime self-knowledge, platform-correct paths, evolution portability."
  :motivation "The agent must never crash. It must know itself and repair itself autonomously."
  :law "Rule 7 (Never crash), Rule 8 (Know thyself). XDG/platform conventions."
  :changes ("Erlang-style supervision: %supervised-action wraps every tick action"
            "Inline tick execution, atomic outbound queue swap"
            "Consecutive error tracking with 5x adaptive cooldown"
            "Gateway FFI hardening: catch_unwind on all frontend calls"
            "Runtime introspection: platform detection, path introspection, library tracking"
            "Error ring buffer: circular 64-entry for self-diagnosis"
            "Self-compilation (%cargo-build-component) and hot-reload (%hot-reload-frontend) (note: hot-reload later replaced by runtime IPC via ractor actor system)"
            "DNA rules 7 & 8 added, system prompt includes self-knowledge block"
            "Platform-correct paths: XDG-style separation of user data, libs, source, logs"
            "Uninstall with evolution safety gate, export/import portability")
  :risk "Platform path migration backward-compatible. handler-case zero-cost on SBCL happy path."
  :rollback "Revert paths.rs; revert loop.lisp; remove introspection.lisp; remove catch_unwind.")

 (:version 6 :date "2026-03-07"
  :target "SignalGuard — Security Kernel + Adaptive Harmonic Shell."
  :motivation "Close critical signal injection, ACE, and confused deputy vulnerabilities."
  :law "Boundary-first safety; LLM output is a proposal, not a command; deterministic gates."
  :changes ("Security kernel: typed signal dispatch, %policy-gate (14 ops), taint propagation"
            "Safe parsers replacing all read-from-string on external data"
            "Boundary wrapping for external data in prompts, memory, search results"
            "Matrix threshold hardening for privileged edges"
            "Gateway dissonance scanning, security-aware routing"
            ":security-audit phase in harmonic state machine"
            "Tailnet HMAC authentication with replay protection"
            "MQTT fingerprint validation"
            "Vault encryption at rest (AES-256-GCM)"
            "Admin intent crate (Ed25519 signatures)")
  :new-crates ("lib/core/signal-integrity" "lib/core/admin-intent")
  :risk "Typed signal dispatch backward-compatible. serde(default) on new MeshMessage fields."
  :rollback "Revert orchestrate-once to string-only dispatch; remove policy gate; restore format-string gateway prompts.")

 (:version 5 :date "2026-03-06"
  :target "A2UI signal protocol, capabilities-driven routing, push integration."
  :motivation "Eliminate hardcoded frontend-name checks; make A2UI generic across any frontend."
  :law "Boundary-first safety; compression as intelligence pressure."
  :changes ("Gateway signal carries metadata (per-message) and capabilities (per-frontend)"
            "Poll format extended to 3-field backward-compatible"
            "Conductor checks signal capabilities (not frontend name) for A2UI dispatch"
            "A2UI component catalog (config/a2ui-catalog.sexp) — 21 components"
            "Push rlib (lib/frontends/push) consumed by mqtt-client"
            "MQTT device registry, offline queue, push notification integration"
            "Gateway node added to matrix topology"
            "Text fallback for A2UI payloads to non-A2UI frontends")
  :risk "3-field poll format backward-compatible; 2-field frontends unaffected."
  :rollback "Revert capabilities/metadata fields to None; restore 2-field parser.")

 (:version 4 :date "2026-03-05"
  :target "Architecture formalization."
  :changes ("Architecture formalized into core/backends/tools/frontends pillars"
            "Gateway/baseband channel became central frontend dispatch path"
            "Tailnet/tailscale channel integrated into mesh-ready communication model"))

 (:version 3 :date "2026-03-05"
  :target "CLI swarm tier."
  :changes ("Parallel-agents gained tmux-driven CLI swarm tier"
            "Multi-agent orchestration expanded beyond API-only subagents"
            "Rewrite execution protocol strengthened with CLI automation"))

 (:version 2 :date "2026-02-18"
  :target "Data-driven policy."
  :changes ("Harmonic matrix and runtime policy moved toward data-driven control"
            "Hardcoded operational policy reduced in favor of .sexp configuration"))

 (:version 1 :date "2026-02-17"
  :target "Runtime scaffold."
  :changes ("Runtime orchestration scaffold stabilized"
            "Uniform C-ABI exports standardized across crates"
            "Shared linkage path enabled across platforms"))

 (:version 0 :date nil
  :target "Genesis."
  :changes ("Human-authored bootstrap corpus and runtime skeleton"
            "DNA constitution anchored"
            "Core loop and CFFI orchestration path established")))
