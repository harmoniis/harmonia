(:title "Ports And FFI"
 :architecture "Port-style architecture in Lisp (src/ports/*.lisp). Each port encapsulates one capability contract and binds to one or more Rust crates."

 :port-map
  ((:port "Vault" :lisp "src/ports/vault.lisp" :rust "lib/core/vault"
    :responsibility "Secret storage and lookup")
   (:port "Store" :lisp "src/ports/store.lisp" :rust "lib/core/config-store"
    :responsibility "Mutable non-secret runtime config")
   (:port "Router" :lisp "src/ports/router.lisp" :rust "lib/backends/llms/provider-router"
    :responsibility "Generic LLM provider router over provider adapters")
   (:port "Matrix" :lisp "src/ports/matrix.lisp" :rust "lib/core/harmonic-matrix"
    :responsibility "Route constraints + telemetry")
   (:port "Ouroboros" :lisp "src/ports/ouroboros.lisp" :rust "lib/core/ouroboros"
    :responsibility "Self-healing crash ledger, patch writing, evolution lifecycle via IPC actor")
   (:port "Baseband" :lisp "src/ports/baseband.lisp" :rust "lib/core/gateway + lib/core/baseband-channel-protocol + frontend modules in harmonia-runtime"
    :responsibility "Unified command dispatch, typed Baseband Channel Protocol envelopes, channel send/status, gateway admin lifecycle")
   (:port "Swarm" :lisp "src/ports/swarm.lisp" :rust "lib/core/parallel-agents"
    :responsibility "Parallel and tmux subagents")
   (:port "Evolution" :lisp "src/ports/evolution.lisp" :rust "lib/core/ouroboros"
    :responsibility "Rewrite prep/execute/rollback")
   (:port "Chronicle" :lisp "src/ports/chronicle.lisp" :rust "lib/core/chronicle"
    :responsibility "Graph-native knowledge base, time-series observability, concept graph SQL traversal")
   (:port "Signalograd" :lisp "src/ports/signalograd.lisp" :rust "lib/core/signalograd"
    :responsibility "Chaos-computing advisory kernel: observe, feedback, checkpoint, restore, status")
   (:port "Signal Integrity" :lisp "(used by gateway + conductor)" :rust "lib/core/signal-integrity"
    :responsibility "Shared injection detection + dissonance scoring")
   (:port "Admin Intent" :lisp "(used by conductor policy gate)" :rust "lib/core/admin-intent"
    :responsibility "Ed25519 admin intent signature verification"))

 :shared-infrastructure
  (:defined-in "src/ports/vault.lisp"
   :utilities
    ("%release-lib-path: resolve release dylib paths."
     "%release-lib-roots: resolve candidate library roots via fallback chain."
     "%split-lines: decode newline-returned ffi outputs."))

 :core-contract "All external effects go through one of these ports. signalograd is a special case — not an external network effect port, but kept behind a port boundary so the adaptive kernel remains explicit, inspectable, and replaceable."
 :guarantees ("traceability in Lisp" "bounded FFI surfaces" "policy enforcement (matrix + vault + config) at orchestration points"))
