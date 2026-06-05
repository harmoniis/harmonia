# Harmonia — Production Roadmap & Verification Handoff

**Status as of 2026-06-05.** This is the canonical "what's true, what's next" doc.
The agent **boots and works end-to-end with a free model** (math, shell tools, memory
store/recall via the TUI). The core math is unit-validated. The remaining work is
architectural depth, not basic function. Read this top-to-bottom before changing code.

---

## 0. Non-negotiable design constraints (from the creator)

- Pure **functional/declarative**; **actor model** everywhere (ractor); **no FFI** (S-expr IPC over a Unix socket only); **no global singletons** (state is actor-owned).
- **No legacy / no TODO / no dual implementations** — one correct path, all callers updated. Working tree must read as if it was always written this way (don't name changes "strip/remove/decouple"; describe the architecture).
- **Homoiconic**: internal representation is code/data (s-expressions), not prose. Only the user prompt (in) and final answer (out) are natural language.
- **Harness ANY model**: even a weak/free model must drive the REPL reliably. Primitives are pre-programmed, obviously named; the frame teaches concrete calls, never lambda-lists.
- **Reduce Kolmogorov complexity without destroying functionality.** Make simple things simple, complex things possible.
- **Closed-loop**: every change is proven by running the agent and reading the trace — never by reading code alone (code-reading produced a false "production-ready" verdict; the runtime disproved it).

---

## 1. The verification harness (USE THIS — it is the spine of all work)

All under `scripts/dev/`. Operate on an **isolated dev node** (`HARMONIA_STATE_ROOT=~/.harmoniis/harmonia-dev`); **never touch the live `~/.harmoniis/harmonia` state** (464 MB of real memory).

| Tool | Purpose |
|---|---|
| `scripts/dev/harness.sh {bringup\|smoke\|repl-test\|teardown\|status}` | Bring up runtime+sbcl on the dev node; drive a TUI prompt; assert a coherent, non-error reply. |
| `scripts/dev/circuit-probe.sh` | End-to-end circuit + observability: sentinel store→recall→DB persistence, trace completeness, scoring-honesty scan, per-subsystem recording. |
| `scripts/dev/deep-probe.lisp` | Headless instrumentation of every signal: eval harness, memory-field attractor/eigenmode/basin, signalograd projection, palace context layers, chronicle tables, IPC + REPL chains. Run via: `sbcl --load src/core/boot.lisp --eval '(harmonia:start :run-loop nil)' --load scripts/dev/deep-probe.lisp --eval '(harmonia::deep-probe-run)'` against a live runtime. |

**Bring-up that survives** (the Bash background gotcha): start `harmonia-runtime` and the sbcl
agent as **persistent background processes** (each in its own returning invocation). A single
shell that starts them *and* keeps running gets its process group SIGTERM'd on exit, killing them.

**cargo on this machine — CRITICAL:** `cargo` resolves to `~/.local/bin/cargo`, a
machine-wide **serialization guard** (atomic mkdir-lock `/tmp/harmoniis-cargo.lock`).
The lock is **shared across ALL projects and concurrent agent sessions**. **NEVER `rmdir`
the lock or `pkill rustc/cargo` broadly** — other sessions compile through it; doing so
causes concurrent compiles → **OOM crash**. Just run `cargo` (the shim) via the Bash tool's
`run_in_background:true` and let it wait on the lock. Build/test is the ONLY serialized gate.

---

## 2. What is VERIFIED working (2026-06-04)

- **Boot & stability**: full stack (runtime + sbcl) boots; `basin: active`, IPC connected; stays up. The historical `sbcl exit 1` crash-loop was environmental (SIGTERM/process-group), not a code fault.
- **Model harness / eval**: task classification, 7-dim task weights (sum=1), tier pools (free=2/eco=8/premium=3/auto=10), `choose-model`, REPL fluency/scoring — all correct live. **Errors now score `code-error`** (was inverted), fluency drops with errors.
- **REPL frame**: teaches concrete calls (`(recall "topic")`, `(store "…")`), no `&key`/`&optional`. Free models converge to the recommended patterns.
- **Weak-model control loop**: repeated completed forms are rejected structurally; observation-only results cannot leak as unrelated answers; `respond` is recognized as an evaluator form; backend exhaustion returns only a useful result or a graceful failure.
- **Memory**: store→chronicle `memory_entries`→recall round-trips (sentinel-verified + DB-verified). `palace-context` tiers L0–L3 work (fixed a case bug). One shared `memory-recall` path unions field and lexical candidates before ranking, so a weak field hit cannot starve an exact stored fact. User-facing recall is concise, ranks explicit stored facts over interaction transcripts, preserves authoritative metadata across palace duplicates, recalls short alphanumeric codes, and no longer leaks `MEMORY_RECALL:` framing.
- **Memory field connectivity and topical recall (P1)**: field graph IPC carries nested s-expression data exactly once, preserving canonical concept/entry IDs and domains. Query concepts are explicit unit boundary conditions; historical access remains a separate signal. Latest live raw recall ranks `memory` first at `1.000` with domain `:cognitive`; the first eight nontrivial eigenvalues are all positive and coherence becomes nonzero after recall.
- **Palace graph construction (P0)**: filing memory builds wings, concepts, typed edges, and inter-domain tunnels. Latest final deep-probe graph after filing: **334 nodes, 2,789 edges, 7 wings, 315 concepts, 7 tunnels**; `palace-context :l0` returns the live wings.
- **Declarative field/status (P7)**: `(field)` and `(status)` are compact, single-line s-expressions computed from live state. Both expose palace readiness; no fixed CHAIN/TOOLS prose is emitted.
- **Core math**: memory-field **42/42** unit tests pass (RK4, spectral solver, heat-kernel, topology, basin, query boundary); the previously validated mempalace 17, signalograd 5, and chronicle 11 remain unchanged. Live attractor is bounded AND evolving over 25 RK4 steps.
- **Chains**: IPC round-trip + `%orchestrate-repl` (computed 6×7=42 through the REPL).
- **Harness process safety**: isolated bring-up/teardown uses validated PID files and exact process IDs. It no longer uses broad `pkill` and refuses to take over an unowned TUI socket.

Fixes already in the working tree (uncommitted): `src/core/{repl-loop,sexp-eval,repl-primitives}.lisp`, `src/ports/mempalace.lisp`, plus a pre-existing staged `lib/core/chronicle/src/tables/memory.rs` sexp-escape fix.

### Latest exact-code verification (2026-06-05 — durable persistence + eval-steering pass)

- `harness.sh smoke`: **pass** (free model → `4`).
- `circuit-probe.sh`: **8/8** (now asserts the on-disk palace journal instead of retired chronicle tables).
- `deep-probe.lisp`: **40/40** (added: 1A reconcile idempotency, 1B snapshot-restore, P4 escalation reaches premium / excludes failed model, L0 genesis idempotency, L4 terraphon reachable).
- `persistence-probe.sh` (NEW): **8/8** — teardown→warm-start keeps palace node/drawer counts IDENTICAL, sentinel survives + is recallable, reconcile idempotent.
- `conversation-probe.sh` (NEW): **5/5 core + 1 tracked gap** — multi-turn store, shell tool, REPL math, cross-turn name+language recall all pass; compound multi-fact recall is the tracked gap above.
- `cognition-probe.lisp` (NEW): **19/19 + 3 tracked gaps**. Precise scope of what passes: (a) **single-step mechanism correctness** — meditation boosts a co-activated edge by the learning rate, bridges at the co-activation threshold, success-boost(2)>failure-boost(1); dream returns structural stats AND does not regress field coherence; evolution accrues samples and moves success-rate UP on success and DOWN on failure (symmetric, not a ratchet); and (b) exactly **one genuinely closed adaptive loop — eval→selection** (15 code-errors on the chosen free model collapsed its fluency to 0.0 and flipped `choose-model` to a different model). Plus: signalograd kernel advances on observation (cycle 0→1) and modulates routing weights clamped [0.05,0.70]; harmonic phase FSM visits every phase once and cycles; task-kind 4/4; domain map accurate on known vocab 8/8. **NOT yet verified: convergence/quality under sustained iteration** — see the unbounded-meditation gap below. Run: `… --load deep-probe.lisp --load cognition-probe.lisp --eval '(harmonia::cognition-probe-run)'`.
- Serialized Rust gate `cargo test -p harmonia-mempalace -p harmonia-chronicle -p harmonia-runtime`: **30/30** (incl. new `test_persistence_survives_reload_without_duplication`); release runtime rebuilt and used by the live probes.
- All work on the isolated dev node (`HARMONIA_STATE_ROOT=~/.harmoniis/harmonia-dev`); live state untouched.
- **Working tree is verified but uncommitted** — awaiting owner approval to commit on a branch.

---

## 3. WHAT NEEDS TO BE BUILT NEXT (prioritized)

### P0 — Palace knowledge-graph construction — VERIFIED IN WORKING TREE
Memory filing now constructs and maintains wings, concept nodes, typed edges, and inter-wing tunnels declaratively from the memory-put flow. Live `palace-graph-stats` and `palace-context :l0` satisfy the probe. Disk persistence remains P3.

### P1 — Concept-graph connectivity / spectral recall — VERIFIED IN WORKING TREE
**Root cause:** the Lisp port serialized graph nodes, edges, query concepts, and access counts into strings, then serialized the enclosing IPC command again. Rust therefore received escaped inner data; concept and entry IDs gained trailing backslashes, domains collapsed to generic, query concepts did not match graph concepts, and the live spectrum/recall became degenerate.
**Build:** the field port now carries nested s-expression data exactly once and preserves canonical domains. The Rust recall core treats query concepts as unit Dirichlet boundary conditions while retaining historical access as an independent signal, so typed actor calls remain topical without hidden optional metadata.
**Live verification:** graph **612 nodes**; first eight reported nontrivial eigenvalues **12.3517, 12.3796, 21.4813, 22.2836, 18.3125, 19.0589, 34.1865, 32.7591**. Raw actor recall for `("memory" "agent")` ranks `memory` first at `1.000`, returns canonical entry IDs, preserves `memory` as `:cognitive`, and raises post-recall coherence to **0.0079**. `deep-probe.lisp` guards all of these invariants.

### P2 — Delegation-success accounting — VERIFIED IN WORKING TREE
Successful `respond`/REPL-completion turns record `:success` via `model-policy-record-outcome`,
which updates each model's `:success-rate`; scoring consumes it (`%seed-score-with-bias`, weight
`w-success` 0.20) — the eval→steering loop is wired, not just observed. Live: recent `delegation_log`
rows show `SUCCESS-PCT 100.0` for the active models; `deep-probe.lisp` asserts a model with
`success-pct > 0.0` after a successful REPL turn. (The 136 pre-fix rows with empty `model_chosen` are
stale historical data, not produced by the current path.)

### P3 — Palace + concept-graph durable persistence — VERIFIED IN WORKING TREE
**Architecture (the owner's correction to an earlier lazy pivot):** chronicle (L2) is the single durable
**system-of-record**; the palace (L3) and field (L1) are projections that are authoritative *in memory*,
persist their own state to disk as a **durable journal** (persist-before-acknowledge, atomic tmp+rename),
and **reconcile against chronicle on warm-start** — so a journal can never silently diverge from, lose,
or duplicate the record.
- **Palace:** `DrawerSource::Memory{entry_id}` keys every drawer to its chronicle entry. `memory-put`
  threads the id → drawer sexp persists it. On boot, `%palace-reconcile-from-memory` files only the
  chronicle entries the palace lacks (entry-id keyed, idempotent — never re-files, so no duplicate
  drawers). Live: first boot logged **"Reconciled 211 missing palace entries from chronicle"**, building
  a 762 KB `graph.sexp` + ~210 drawer sexps under `<state-root>/mempalace/`.
- **Field:** `chronicle-latest-graph-snapshot` + `%merge-graph-snapshot-into-field` restore runtime-learned
  edges (`:meditation` Hebbian bridges) the entry-rebuild can't reproduce — merged by edge key (max
  weight, union reasons) before push — lossless **up to the last snapshot** (edges learned after the
  most recent `graph_snapshots` row are still lost on an unclean crash; snapshots are also edge-limit
  truncated ~300), without introducing a second source of truth.
- **Removed (no-dual/no-dead):** the `#[deprecated]` chronicle palace persist/load path, the json codebook
  serializer, dead `persist`/`persist_all`/`write_state`/`DrawerStore::restore/pop`.
- **Guards:** crate `test_persistence_survives_reload_without_duplication`; `deep-probe.lisp` reconcile +
  snapshot-restore idempotency; `persistence-probe.sh` (teardown→warm-start: identical counts, sentinel
  survives + recallable, idempotent); `circuit-probe.sh` P4 asserts the on-disk journal.

### P4 — Model escalation reaches premium — VERIFIED IN WORKING TREE
`model-escalation-chain` no longer has the dead `(or (member …) chain)` conditional. It now returns the
ranked alternatives **after** the failed model (never retrying it) then the premium pool — so repeated
REPL failure escalates capability while the default `choose-model` stays free for the common case (this
is the correct mechanism for "reach premium when warranted," not changing the default scoring). Live:
escalation after the free default yields `(claude-opus-4.6 grok-4.1-fast grok-4.20)`; `deep-probe.lisp`
asserts the chain excludes the failed model and intersects the premium tier.

### P7 boundary hardened — tool-diagnostic envelopes (found + fixed 2026-06-05)
`conversation-probe.sh` T6 surfaced a raw `(search: no results …)` observation leaking to the user as the
final answer — a counterexample to P7 ("internal envelopes guarded"). Root cause: `search`/`grep` aren't
in `*repl-observation-operators*`, so a primitive's failure envelope became the answer. **Fixed
structurally** at the answer boundary: `%repl-diagnostic-envelope-p` detects a whole-response
`(<lowercase-word>: …)` envelope (colon after the word — distinct from prose, real calls `(search "x")`,
and keyword plists `(:status …)`) and `%sanitize-repl-response` converts it to a graceful line. `circuit-probe`
8/8 (normal recall unaffected) and `conversation-probe` T6 now asserts no envelope leaks. P7 holds again.

### Known gap — compound multi-fact recall (found 2026-06-05)
`conversation-probe.sh` T6: a single question asking for two separately-stored facts ("what language AND
what database") makes the free model emit one over-literal `search` query that misses — even though the
facts ARE stored (T5 recalls name+language fine). The raw-envelope leak is fixed (above); what remains is a
recall-**quality** gap: the compound query does not decompose into both facts. Next: decompose multi-fact
recall queries. (Separable from the boundary, which is now guarded.)

### Known gaps — cognitive machinery (found 2026-06-05 by `cognition-probe.lisp`)
What is *proven*: every self-improvement mechanism is single-step correct, and one adaptive loop (eval→selection) genuinely closes. What is *not* proven is convergence under sustained iteration — and the first gap is exactly a runaway there. Three gaps:
0. **Meditation edge weight is UNBOUNDED (runaway risk).** `%strengthen-edges` does `(+ weight boost)` with no cap or decay; 50 co-activations of one pair grew its weight 1→101 (linear). Because `memory-map-sexp` sorts edges by weight and truncates to ~300 (the input to the 1B field warm-start merge), a few hot edges will crowd everyone else out of the snapshot, and the skewed weight distribution re-creates the degenerate spectrum (eigenvalues→0) that P1 fixed. The field attractor has a boundedness test; the *learning* weights do not. Fix: add saturation (e.g. soft cap / logarithmic boost) or decay to the meditation rule, then assert convergence in the probe.
1. **Domain classification is a hardcoded keyword table, blind to the agent's own technical vocabulary.** `%concept-domain` (concept-map.lisp) maps a small fixed word list; **12/12** probed technical terms (`rk4 lorenz attractor ractor ipc eigenvalue laplacian spectral actor sexp chronicle signalograd`) fall to `:generic`. So the memory-field domain signal — which drives inter-domain tunnels and domain-aware recall — is degenerate for exactly the engineering/math content Harmonia works with. Fix: derive domain from the concept-graph neighbourhood / co-occurrence (or a learned/extensible map), not a static list.
2. **Harmonic-matrix is an open loop for model selection.** `harmonic-matrix-observe-route` records edge use/success/latency, but `%selection-chain-tiered` never consults `harmonic-matrix-route-allowed-p` / matrix experience — so route history does not yet steer which model/tool is chosen. Fix: gate or bias selection on matrix experience to close the loop.

### P5 — Dead config wiring (no dead paths allowed)
**Probed dead/unread config:** `:task-tier-hints`, `:model-boosts`, `:cascade-config` in `config/model-policy.sexp` are defined but never read. Either wire them or remove them (no dead config). (`:model-bans`, cli-cooloff, cli-preference ARE wired — leave them.)

### P6 — `search-exa` / `search-brave` broken + hardcoded vault path
**Symptom (boot warning):** both embed `~/.harmoniis/...vault.db` inside a FORMAT control string — `~/` is the FORMAT "call-function" directive (→ `Undefined function .HARMONIIS`), and they read the raw vault.db file as if it were a key. **Web search tools are broken.**
**Build:** read the API key from the vault via the proper port (not by cat-ing vault.db), and build the curl command without FORMAT directive collisions (escape `~` or use concatenation). Honor "no hardcoded paths" — derive from state-root/config.

### P7 — Declarative presentation/recall response — VERIFIED IN WORKING TREE
`(field)`, `(status)`, and user-facing recall now emit compact computed/declarative values. Raw drawers, fixed CHAIN/TOOLS prose, internal recall envelopes, and pretty-printer line wrapping are guarded by deterministic tests plus the TUI/circuit probes.

### P8 — Tailscale frontend is a `OnceLock` singleton
Migrate `lib/frontends/tailscale/src/bridge.rs` from the `OnceLock<RwLock<…>>` free-function singleton to a proper `Frontend` trait actor like every other frontend (no singletons).

### P9 — Frontends to production (email / http3 / sip / mqtt)
All are wired and spawn as actors, but only TUI is fully testable locally. Build **correctness pass + integration tests with local mocks** (mock IMAP/SMTP, in-proc `rumqttd`, loopback SIP peer, in-proc HTTP/3) + a documented live-smoke procedure. Verify each `inbound → signal → core → response → outbound` path.

### P10 — `*ROUTING-RULES-SEXP*` load-order compile warning
`model-providers.lisp` (loaded before `model-routing.lisp`) references `*routing-rules-sexp*` → compile warning. It's bound by runtime (cosmetic), but fix the load order / forward-declare for cleanliness.

### Housekeeping
Commit the verified working-tree fixes (REPL/sexp-eval/primitives/mempalace + the staged chronicle `sexp_escape` fix) on a branch once the owner approves.

---

## 4. Definition of done

For each item: change → serialized `cargo test ...` + `cargo build --release ...` through the shared shim → `scripts/dev/harness.sh smoke` → `scripts/dev/circuit-probe.sh` (7/7) → `deep-probe.lisp` (no regressions). The agent must keep working end-to-end with a **free model** at every step. Add a probe/assertion for each fixed item so the closed loop guards it.

See also: the approved plan `/Users/george/.claude/plans/read-harmoniis-agent-harmonia-and-bring-optimized-pearl.md`, and `doc/hardcoded-prompts-audit.md`.
