(:title "Rewrite Roadmap"
 :purpose "High-leverage evolution targets while preserving genesis alignment."

 :priorities
  ((:priority 1 :name "Stability And Observability"
    :done ("Erlang-style supervision (v7)" "Error ring buffer (v7)" "Library crash tracking (v7)"
           "catch_unwind on all gateway FFI calls (v7)" "Adaptive cooldown (v7)"
           "Runtime self-knowledge in DNA system prompt (v7)"
           "Chronicle knowledge base with 9 tables (v8)" "Arbitrary SQL query (v8)"
           "Concept graph decomposition with CTE traversal (v8)"
           "Pressure-aware GC (v8)" "Harmony trajectory (v8)" "A2UI dashboard (v8)")
    :remaining ("Extend matrix time-series usage in runtime decisions"
                "Improve structured error taxonomy and recovery summaries"
                "Keep boot-to-ready path deterministic under partial frontend availability"))

   (:priority 2 :name "Token And Tool Efficiency"
    :remaining ("Expand codemode pipelines for multi-step deterministic operations"
                "Reduce unnecessary LLM relay when tool plans are predictable"
                "Use policy feedback loops to tune model selection for task classes"))

   (:priority 3 :name "Memory Quality"
    :remaining ("Improve crystallization heuristics for decision-heavy sessions"
                "Increase topic coherence in temporal journal generation"
                "Evaluate edge quality in concept graph for better rewrite focus"))

   (:priority 4 :name "Evolution Safety"
    :done ("Evolution export/import (v7)" "Uninstall safety gate (v7)"
           "Hot-reload (v7)" "Self-compilation (v7)")
    :remaining ("Standardize rewrite preflight checks"
                "Require explicit rollback plans in automated patch proposals"
                "Tie rewrite acceptance to measurable score delta"))

   (:priority 5 :name "Signal And Channel Maturity"
    :done ("Capabilities-driven A2UI dispatch (v5)" "Gateway signal metadata enrichment (v5)"
           "Push notification integration (v5)" "MQTT device registry (v5)"
           "A2UI component catalog (v5)"
           "Unified command dispatch: gateway as single interception point for all /commands (v10)"
           "All crate Cargo.toml unified to rlib (cdylib removed, FFI replaced by IPC) (v10)")
    :remaining ("Extend capabilities to non-A2UI features (:voice, :location, :accessibility)"
                "Frontend capability negotiation at connect time"
                "A2UI catalog versioning for compatibility"))

   (:priority 6 :name "Security Hardening"
    :done ("SignalGuard security kernel (v6)" "Safe parsers (v6)" "Boundary wrapping (v6)"
           "Matrix threshold hardening (v6)" "Gateway dissonance scanning (v6)"
           "Security-aware routing (v6)" ":security-audit phase (v6)"
           "Tailnet HMAC (v6)" "MQTT fingerprint validation (v6)"
           "Vault encryption at rest (v6)" "signal-integrity crate (v6)"
           "admin-intent crate (v6)")
    :remaining ("Tamper-evident security ledger"
                "Behavioral baseline + anomaly detection"
                "Daily security digest generation"
                "Frontend capability negotiation for dynamic security label upgrade"))

   (:priority 7 :name "Platform Maturity"
    :done ("Platform-correct path structure (v7)" "XDG-style paths (v7)"
           "Platform-specific runtime dirs (v7)" "Platform-specific log dirs (v7)"
           "Library path fallback chain (v7)")
    :remaining ("Windows support testing" "FreeBSD service file testing")))

 :scope-exclusions
  ("bypass matrix route checks"
   "expose vault values in memory/logging"
   "weaken security kernel invariant guards"
   "allow *read-eval* on external data paths"
   "bypass policy gate for privileged operations"
   "remove taint propagation from signal dispatch"
   "weaken DNA validation at startup"))
