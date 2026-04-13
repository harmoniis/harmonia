# Session Handoff — 2026-04-13

## CRITICAL: Read This First

This document is the COMPLETE context for the next development session on Harmonia. The previous session (2026-04-12/13) made 16 commits that fixed real bugs BUT also introduced violations of the codebase design patterns. ALL violations must be fixed before any new features are built.

## Codebase Rules — ETERNAL COMMANDMENTS

These apply to EVERY change in this repository. Violating any of these is unacceptable.

### 1. Pure Functional Declarative
- No mutable global state outside actor-owned state
- No imperative loops where functional alternatives exist
- No side effects in handler functions — side effects only in `apply()`
- Every function is a pure transformation: input → output

### 2. Actor Model as Law
- ALL inter-component communication through actor messages
- NO synchronous blocking IPC calls inside primitives
- State owned exclusively by actors, accessed only through messages
- Supervision trees for fault tolerance (Erlang/OTP pattern)

### 3. Free Monad / Service Pattern
- Commands are pure data (Cmd enum)
- Handlers are pure functions: `handle(&self, cmd) → (Delta, Ok)`
- Mutation confined to `apply(&mut self, delta)` — the ONE mutation point
- Dispatch through ComponentDescriptor trait

### 4. Homoiconic — Code is Data, Data is Code
- Configuration as s-expressions, not hardcoded constants
- Prompts as s-expression structures, rendered at boundaries
- Primitive registration as data declarations, not code
- Everything evolvable by the epigenetic system

### 5. Declarative Policy, Not Code Logic
- Memory routing: `config/memory-routing.sexp`, not `(member class '(:soul :skill))`
- Tool registration: data table, not case statement
- Model selection: scoring functions + config weights, not if/else
- Prompt structure: s-expression template, not string concatenation

### 6. Metaprogramming / Macro Declarative
- `defprimitive` macro for primitive registration
- `declare_component!` macro for Rust actors (already exists)
- Dispatch tables generated from declarations, not hand-coded
- New capabilities added by DECLARING them, not by editing switch statements

### 7. Kolmogorov Complexity Reduction
- Compress information, never destroy it
- Prefer abstraction over repetition
- Every new function must reduce total codebase complexity
- If adding code increases complexity without adding capability, it's wrong

### 8. Traits and Protocols
- Shared interfaces for all components (ComponentDescriptor, Service)
- Primitives share a protocol: `(name args-spec doc handler)`
- Tools share a protocol: `(name capabilities invoke)`
- Memory layers share a protocol: `(store recall query)`

### 9. No Hardcoding, No Heuristics, No Workarounds
- If something doesn't work, fix the root cause
- No string matching for content filtering
- No hardcoded file paths — derive from state-root/config
- No hardcoded class lists — use policy files
- The protocol is unambiguous — if models fail, downgrade them

### 10. The Agent Never Fails
- It figures out with the model how to repair itself
- Either choose another model, or rewrite itself via REPL
- DNA constraints are immutable; epigenetics tunable within bounds
- Evolution only when truly necessary, gated by vitruvian signal/noise

## What Was Done Right (keep these)

### Rust side (properly designed)
- `lib/core/signalograd/src/lib.rs` — Service trait impl for KernelState (SignalogradCmd/Delta/Ok)
- `lib/core/runtime/src/dispatch/signalograd.rs` — Service pattern dispatcher
- `lib/core/signalograd/src/checkpoint.rs` — atomic temp+rename writes
- `lib/core/phoenix/src/supervisor.rs` — safe child access in crash handler
- `lib/core/runtime/src/dispatch/matrix.rs` — field name compatibility (:node/:node-id/:id)
- `lib/core/runtime/src/dispatch/workspace.rs` — shell exec for compound commands
- `lib/core/runtime/src/dispatch/mod.rs` — shared esc() function
- `config/model-policy.sexp` — ber1 demoted, gemini primary, model-bans

### Critical bug fixes (correct)
- `*package*` binding in `%eval-all-forms` — symbols must read in :harmonia package
- `ipc-extract-value` escaped quote handling — was truncating at first `\"`
- `%bound-result` removed from `%reval-call` — data flows through let bindings intact
- `memory-field-load-graph` wired at boot — graph was never pushed to Rust engine
- Signalograd observation already includes `:repl-fluency` and `:repl-speed`

### Architecture decisions (correct)
- Memory layer separation: L0 boot, L1 field, L2 chronicle, L3 palace, L4 datamine
- Memory field = global context graph (not data store)
- Chronicle = system self-log (not user data)
- Palace = user knowledge (organized by domain)
- Boot prompt teaches model to call (field) first for chain-of-thought

## 12 Violations That Must Be Fixed

### V1: Giant case statement — sexp-eval.lisp
- 41 primitives in one `case` expression
- FIX: `defprimitive` macro + hash-table dispatch

### V2: 41 ad-hoc %prim-* functions — repl-primitives.lisp
- No shared protocol, different signatures
- FIX: Primitive protocol struct `(name args-spec doc handler)`

### V3: Hardcoded *repl-frame* — repl-loop.lisp:139
- Static string listing primitives
- FIX: Compute from primitive dispatch table at runtime

### V4: Hardcoded (field) response — repl-primitives.lisp
- CHAIN, TOOLS, MEMORY sections are format strings
- FIX: Derive from registered primitives + memory state + config

### V5: Hardcoded class routing — operations.lisp
- `%field-indexable-p`: `(member class '(:soul :skill :genesis))`
- `%palace-worthy-p`: `(member class '(:daily :interaction))`
- FIX: `config/memory-routing.sexp` policy file

### V6: Response sanitizer with string matching — repl-loop.lisp
- `(search "harmonia" ...)` literal agent name
- FIX: Structural: strip `;;` comment lines only

### V7: 13 synchronous IPC calls — repl-primitives.lisp
- `ipc-call` and `workspace-exec` block the REPL
- FIX: Async actor messages through ractor system

### V8: Hardcoded temp file paths — repl-primitives.lisp
- `/tmp/harmonia-fetch.py`, `/tmp/harmonia-py-exec.py`
- FIX: Derive from state-root via config-store

### V9: Memory routing in code — operations.lisp
- Which class goes to which layer decided by Lisp functions
- FIX: Declarative policy in `config/memory-routing.sexp`

### V10: Prompt assembly not homoiconic — repl-loop.lisp
- String concatenation, not s-expression structure
- FIX: Prompt as s-expr, rendered at boundary

### V11: Fetch primitive generates Python at runtime — repl-primitives.lisp
- Format string with URL substitution, written to temp file
- FIX: Use existing Rust browser/http crate through IPC

### V12: Palace room mapping still has class→domain hardcoding — mempalace.lisp
- `%palace-room-for-class` does `(string-downcase (symbol-name class))`
- This is actually OK (generic enough) but rooms should be policy-driven

## Test Cases That Must Pass

### Basic REPL circuit
1. Boot → (field) → model gets context → (basin) → (respond answer) — all in 2 rounds
2. (exec "uname -a") returns clean output without STDERR noise
3. (let ((x (exec "sw_vers"))) (store x) (respond (str "stored " (length x) " chars"))) — x holds FULL content

### Memory layers
4. Genesis entries → L1 field concepts (vitruvian, kolmogorov, lorenz)
5. User interactions → L3 palace drawers (searchable)
6. Tool metrics → L2 chronicle only (no field pollution)
7. Palace search returns real results

### Web fetching
8. (fetch "https://bravors.brandenburg.de/br2/sixcms/media.php/76/GVBl_I_08_2025.pdf") → returns law text
9. Content flows through let bindings intact (not truncated at 1500)
10. Store receives full content, not truncated

### Model scoring
11. Model that writes valid sexp gets fluency boost
12. Model that writes broken sexp gets fluency penalty
13. Model rotation selects by score, not random

### System discovery
14. (exec "sw_vers") + (exec "sysctl -n hw.memsize") + (exec "which rustc python3") — all in one round
15. Results stored to palace, searchable via (recall)

## Files Modified in This Session

| File | Lines Changed | Status |
|------|--------------|--------|
| `src/core/sexp-eval.lisp` | +30/-10 | NEEDS REDESIGN (V1, V2) |
| `src/core/repl-loop.lisp` | +80/-60 | NEEDS REDESIGN (V3, V6, V10) |
| `src/core/repl-primitives.lisp` | +120/-30 | NEEDS REDESIGN (V2, V4, V7, V8, V11) |
| `src/core/pipeline-trace.lisp` | NEW | OK (trace path fix needed) |
| `src/core/system-commands.lisp` | +84 | OK |
| `src/core/boot.lisp` | +56 | Partially OK (V5 routing) |
| `src/core/signalograd.lisp` | +20 | OK |
| `src/core/model-routing.lisp` | +15/-5 | OK (demoted filter) |
| `src/core/model-providers.lisp` | +5 | OK |
| `src/core/supervisor.lisp` | +2/-3 | OK |
| `src/dna/dna.lisp` | +1/-1 | OK (:genesis class) |
| `src/memory/store/operations.lisp` | +80/-30 | NEEDS REDESIGN (V5, V9) |
| `src/memory/store/concept-map.lisp` | +5/-5 | OK (tag→content edges) |
| `src/memory/store/state.lisp` | +5 | OK (depth inference) |
| `src/ports/mempalace.lisp` | +30 | NEEDS REDESIGN (V12) |
| `src/ports/ipc-ports.lisp` | +15/-10 | OK (escaped quote fix) |
| `src/ports/ipc-client.lisp` | +1/-13 | OK (dedup removal) |
| `src/ports/router.lisp` | +8/-2 | OK (trace) |
| `src/orchestrator/prompt-assembly.lisp` | +3/-2 | OK |
| `src/orchestrator/security.lisp` | +8/-8 | OK (paren fix) |
| `config/model-policy.sexp` | full rewrite | OK |
| `lib/core/signalograd/src/lib.rs` | +123 | OK (Service trait) |
| `lib/core/signalograd/Cargo.toml` | +1 | OK |
| `lib/core/signalograd/src/checkpoint.rs` | +3/-2 | OK (atomic write) |
| `lib/core/runtime/src/dispatch/signalograd.rs` | rewrite | OK |
| `lib/core/runtime/src/dispatch/matrix.rs` | +5/-80 | OK |
| `lib/core/runtime/src/dispatch/tailnet.rs` | +25/-1 | OK |
| `lib/core/runtime/src/dispatch/workspace.rs` | +15/-5 | OK |
| `lib/core/runtime/src/dispatch/mod.rs` | +5 | OK |
| `lib/core/runtime/src/dispatch/provider_router.rs` | +2/-5 | OK |
| `lib/core/runtime/src/spawn.rs` | +8/-5 | OK |
| `lib/core/runtime/src/actors/mod.rs` | +1 | OK |
| `lib/core/phoenix/src/supervisor.rs` | +10/-8 | OK |
| `lib/core/gateway/src/sender_policy.rs` | +8/-3 | OK |
| `lib/core/memory-field/src/lib.rs` | +3/-2 | OK |
| `lib/core/ouroboros/tests/integration.rs` | +2/-1 | OK |
| `lib/backends/llms/openrouter/src/client.rs` | +4/-2 | OK |

## User Expectations

The creator expects:
1. Pure functional declarative code everywhere — Lisp AND Rust
2. Actor model as the ONLY communication pattern
3. Metaprogramming (macros, dispatch tables) instead of switch statements
4. Homoiconic design — code and data interchangeable
5. Policy-driven configuration — not logic in code
6. Kolmogorov complexity reduction — less code, more capability
7. Working e2e use cases — not faked demos
8. Honest reporting — no hiding failures
9. The agent handles 100 different use cases, not just the ones tested
10. Browser, markitdown, tmux, datamining all accessible via REPL
11. Memory field = global context map, not data dump
12. Chronicle = system log, Palace = user knowledge
13. Models guided through chain-of-thought, not flooded with data
