(:title "Current State"
 :snapshot-date "2026-03-11"

 :active-evolution-mode
  (:current :artifact-rollout
   :source-rewrite nil
   :source "src/ports/evolution.lisp"
   :operations
    ("evolution-prepare inspects health and crash state"
     "evolution-execute signals artifact rollout under Phoenix or writes patch artifacts when source rewrite is enabled"
     "evolution-rollback records rollback as crash telemetry"))

 :runtime-readiness
  (:signals ("harmonic convergence (global/local + lambdoma ratio)"
             "logistic chaos risk thresholds"
             "vitruvian signal/noise gates")
   :thresholds-source "config/harmony-policy.sexp"
   :threshold-keys ("rewrite-plan/signal-min" "rewrite-plan/noise-max" "rewrite-plan/chaos-max"))

 :model-swarm-policy
  (:description "Model selection is task-aware, provider-scoped, and evolves from measured delegation outcomes."
   :inputs ("config/model-policy.sexp" "config/swarm.sexp" "mutable state files under HARMONIA_STATE_ROOT")
   :seed-source "config-store first (no hardcoded runtime seed lock)"
   :seed-keys ("model-policy/provider" "model-policy/seed-models" "model-policy/seed-models-<provider>")
   :openrouter-default-seed ("inception/mercury-2" "qwen/qwen3.5-flash-02-23" "minimax/minimax-m2.5" "google/gemini-3.1-flash-lite-preview")
   :cli-routing (:active-for (:software-dev :coding :critical-reasoning)
                  :controls (:cli-timeout-seconds :cli-cooloff-seconds :cli-quota-patterns))
   :orchestrator-mode "coordinator-first: non-tool LLM work delegated through swarm (parallel-solve)"
   :context-summarizer (:model "qwen/qwen3.5-flash-02-23" :threshold :context-summarizer-threshold-chars)
   :outcome-persistence ("swarm_model_scores.sexp" "chronicle.db delegation_log table"))

 :memory-evolution
  (:layers ("Soul seeding from DNA"
            "Daily interaction memory"
            "Skill compression and crystallization"
            "Temporal journaling (yesterday summary)")
   :thresholds-source "config/harmony-policy.sexp :memory section")

 :signalograd
  (:status "active adaptive runtime"
   :architecture ("Lorenz-style chaotic reservoir / plastic CTRNN regime as temporal compute"
                  "Hopfield-like attractor memory storing compressed successful states"
                  "tiny bounded readout heads for harmony, routing, memory, evolution, and security shell"
                  "local online learning only: Hebbian, Oja-style normalization, decay, homeostatic control")
   :operational-rules ("telemetry-first inputs only in v1"
                       "no raw prompt text as model input"
                       "advisory output only"
                       "deterministic policy, matrix constraints, and privileged security rules remain sovereign")
   :persistence (:live "HARMONIA_STATE_ROOT/signalograd.sexp"
                  :evolution-checkpoint "src/boot/evolution/latest/signalograd.sexp"))

 :gateway-signal-protocol
  (:enrichment-layers
    ((:name "Capabilities" :type "static per-frontend" :source "config/baseband.sexp :capabilities")
     (:name "Metadata" :type "dynamic per-message" :source "third tab-field in poll output"))
   :poll-format "3-field backward-compatible: sub_channel TAB payload [TAB metadata]"
   :a2ui-dispatch "capabilities-driven — conductor checks signal capabilities, not frontend names"
   :a2ui-catalog (:source "config/a2ui-catalog.sexp" :components 21)
   :push "lib/frontends/push consumed by mqtt-client for offline device push")

 :unified-command-dispatch
  (:version "v10"
   :description "Gateway is the single interception point for ALL /commands from ALL frontends."
   :source "lib/core/gateway/src/command_dispatch.rs"
   :tiers ((:name "native" :commands ("/wallet" "/identity" "/help") :handler "Rust")
           (:name "delegated" :commands ("/status" "/backends" "/frontends" "/tools" "/chronicle" "/metrics" "/security" "/feedback" "/exit") :handler "Lisp callback"))
   :security "Gateway enforces security labels before dispatch: Owner/Authenticated for read-restricted, TUI-only for /exit."
   :callback "CommandQueryFn registered by Lisp during init-baseband-port via harmonia_gateway_set_command_query."
   :exit-handling "Gateway sets pending_exit flag on /exit. Lisp checks via harmonia_gateway_pending_exit after each poll."
   :crate-types "All crates unified to rlib (compiled into harmonia-runtime; cdylib removed, FFI replaced by IPC).")

 :matrix-enforcement
  (:description "All critical orchestrator routes are matrix-gated before invocation."
   :seed "config/matrix-topology.sexp"
   :mutable "HARMONIA_STATE_ROOT/matrix-topology.sexp")

 :security-kernel
  (:status "active (v6)"
   :typed-signal-dispatch
    (:description "External signals are harmonia-signal structs. Conductor dispatches via etypecase."
     :handlers ((:type "harmonia-signal" :handler "orchestrate-signal" :description "boundary-wraps payload, proposed tool commands pass policy gate")
                (:type "string" :handler "orchestrate-prompt" :description "internal/TUI, may contain tool commands directly")))
   :policy-gate
    (:description "Deterministic binary gate protecting 14 privileged operations."
     :operations ("vault-set" "vault-delete" "config-set" "harmony-policy-set" "matrix-set-edge"
                  "matrix-set-node" "matrix-reset-defaults" "model-policy-upsert" "model-policy-set-weight"
                  "codemode-run" "git-commit" "self-push" "parallel-set-width" "parallel-set-price")
     :rules ("Non-privileged ops: always allowed"
             "Privileged ops with tainted origin: denied"
             "Privileged ops from non-owner/non-authenticated: denied"
             "Privileged ops from owner/authenticated + internal taint: allowed"))
   :taint-propagation "*current-originating-signal* set by orchestrate-signal. Nil means owner trust."
   :safe-parsers ("%safe-parse-number: validates [0-9.eE+-] only, *read-eval* nil"
                  "%safe-parse-policy-value: rejects #. reader macros, validates safe types")
   :vault-security ("Encrypted at rest with AES-256-GCM"
                    "Root derived from Harmoniis wallet slot family vault"
                    "Component-scoped read via get_secret_for_component")
   :invariant-guards ("vault min_harmony >= 0.30"
                      "dissonance-weight >= 0.05"
                      "cannot disable injection scanning"
                      "cannot enable *read-eval* on external data")
   :security-posture (:states (:nominal :elevated :alert)
                       :updated-by ":security-audit phase in harmonic state machine")
   :adaptive-shell ("Gateway signals carry dissonance score from injection scanning"
                    "Harmonic matrix route_allowed_with_context with security_weight and dissonance"
                    "Search tool results boundary-wrapped"
                    "Memory recalls boundary-wrapped")
   :config "config/harmony-policy.sexp :security section")

 :fault-tolerance
  (:version "v7"
   :supervision
    (:wrapper "%supervised-action catches serious-condition, records to error ring, never propagates"
     :tick-execution "inline, no intermediate list allocation"
     :outbound-queue "atomic swap: grab-and-clear"
     :error-tracking "consecutive error tracking with 5x adaptive cooldown"
     :outer-handler "run-loop handler-case — the loop truly never crashes")
   :gateway-ffi ("catch_unwind on all frontend FFI calls"
                 "gateway-reload: shutdown, drop library, reload, re-init"
                 "per-frontend crash counts via atomic counter")
   :self-knowledge
    (:source "src/core/introspection.lisp"
     :features ("Platform detection (macOS, Linux, FreeBSD, Windows)"
                "Path introspection (state root, source root, lib dir, log path)"
                "Library tracking via *loaded-libs* hash table"
                "Error ring: circular 64-entry buffer"
                "Self-compilation via %cargo-build-component"
                "Runtime actor reload via ractor supervisor (replaced former %hot-reload-frontend)"
                "Full diagnostic snapshot via introspect-runtime"
                "DNA integration via %runtime-self-knowledge"))
   :platform-paths
    ((:category "User data" :path "~/.harmoniis/harmonia/" :contents "vault.db, config.db, metrics.db, config/, frontends/, state/")
     (:category "Libraries" :path "~/.local/lib/harmonia/" :contents "rlib modules (compiled into harmonia-runtime)")
     (:category "App data" :path "~/.local/share/harmonia/" :contents "Lisp source, docs, genesis, evolution knowledge")
     (:category "Binary" :path "~/.local/bin/harmonia" :contents "CLI binary")
     (:category "Logs" :path "~/Library/Logs/Harmonia/ (macOS)" :contents "harmonia.log")
     (:category "Runtime" :path "$TMPDIR/harmonia/ (macOS)" :contents "PID file, Unix socket"))
   :evolution-portability
    ("Uninstall checks evolution safety before proceeding"
     "Verifies source committed and pushed to git remote"
     "evolution-export creates portable tar.gz archive"
     "evolution-import restores into fresh install"))

 :chronicle
  (:version "v8"
   :description "Graph-native SQLite store recording harmonic evolution, concept graphs, delegation decisions, memory events, and recovery lifecycle."
   :database "SQLite WAL-mode at HARMONIA_STATE_ROOT/chronicle.db"
   :tables 9
   :concept-graph-features ("N-hop reachability via recursive CTE" "Interdisciplinary bridges"
                            "Domain distribution" "Central concepts" "Graph evolution trajectory")
   :arbitrary-sql "chronicle-query runs any SELECT/WITH SQL returning s-expression results"
   :gc-tiers ((:tier :soft :threshold-mb 50 :action "Thin old normal-signal data")
              (:tier :hard :threshold-mb 150 :action "Aggressive thinning, keep only inflection points")
              (:tier :critical :threshold-mb 300 :action "Emergency pruning of all but high-signal rows"))
   :preserved "Inflection points (high chaos_risk > 0.7, rewrite_ready, failed/recovery events). harmony_trajectory never pruned."
   :integration ("harmonic machine :stabilize phase" "memory compression events" "conductor delegation decisions"
                 "Phoenix supervisor lifecycle" "Ouroboros crash/patch events")
   :a2ui-dashboard "chronicle-dashboard-json: 8-panel Composite")

 :memory-field
  (:status :active
   :description "Graph Laplacian field propagation engine for dynamical memory recall with attractor basin assignment and hysteresis."
   :graph-reload :per-harmonic-cycle
   :attractors (:thomas :aizawa :halvorsen)
   :basin-persistence :chronicle
   :warm-start :from-chronicle
   :feedback-to-signalograd :active
   :spectral-modes 8
   :activation-scoring "0.40×field + 0.30×eigenmode + 0.20×basin + 0.10×access (warm-up ramp first 10 cycles)"
   :integration ("harmonic machine :observe pushes graph" ":attractor-sync steps attractors"
                 ":stabilize persists basin to chronicle" "signalograd receives field metrics")))
