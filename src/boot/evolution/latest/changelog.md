# Changelog

Append-only evolution ledger.

## v0 — Genesis

- Human-authored bootstrap corpus and runtime skeleton.
- DNA constitution anchored.
- Core loop and CFFI orchestration path established.

## v1 — 2026-02-17

- Runtime orchestration scaffold stabilized.
- Uniform C-ABI exports standardized across many crates.
- Shared linkage path enabled across platforms.

## v2 — 2026-02-18

- Harmonic matrix and runtime policy moved toward data-driven control.
- Hardcoded operational policy reduced in favor of `.sexp` configuration.

## v3 — 2026-03-05

- Parallel-agents gained tmux-driven CLI swarm tier.
- Multi-agent orchestration expanded beyond API-only subagents.
- Rewrite execution protocol strengthened with CLI automation capabilities.

## v4 — 2026-03-05

- Architecture formalized into core/backends/tools/frontends pillars.
- Gateway/baseband channel became central frontend dispatch path.
- Tailnet/tailscale channel integrated into mesh-ready communication model.

## v5 — 2026-03-06

- Target: A2UI signal protocol, capabilities-driven routing, push integration.
- Motivation: Eliminate hardcoded frontend-name checks; make A2UI generic across any frontend.
- Law/Principle Applied: Boundary-first safety; compression as intelligence pressure (capabilities in config, not code).
- Changes:
  - Gateway Signal carries `metadata` (per-message) and `capabilities` (per-frontend from baseband config).
  - Poll format extended to 3-field backward-compatible: `sub_channel\tpayload[\tmetadata]`.
  - FrontendHandle stores parsed capabilities from `:capabilities (...)` in config sexp.
  - Conductor checks signal capabilities (not frontend name) for A2UI dispatch.
  - A2UI component catalog (`config/a2ui-catalog.sexp`) — 21 components injected into LLM context.
  - Push-sns replaced with generic push rlib (`lib/frontends/push`) consumed by mqtt-client.
  - MQTT frontend gained device registry, offline queue, and push notification integration.
  - Loop.lisp fixed: `:frontend`/`:sub-channel` properly extracted from nested `:channel` plist.
  - Gateway node added to matrix topology.
  - Text fallback extraction for A2UI payloads sent to non-A2UI frontends.
- Risk Notes: 3-field poll format is backward compatible; 2-field frontends unaffected.
- Rollback Plan: Revert capabilities/metadata fields to None; restore 2-field parser.

## v6 — 2026-03-07

- Target: SignalGuard — Security Kernel + Adaptive Harmonic Shell.
- Motivation: Close critical signal injection, arbitrary code execution, and confused deputy vulnerabilities.
- Law/Principle Applied: Boundary-first safety; LLM output is a proposal, not a command; deterministic gates for privileged ops.
- Changes:
  - **Security Kernel (deterministic, non-bypassable)**:
    - `harmonia-signal` struct replaces format-string prompts for external signals (typed signals end-to-end).
    - `%policy-gate` — deterministic binary gate for 14 privileged operations (vault-set, config-set, harmony-policy-set, matrix-set-edge, etc.). Checks taint chain and security label. Blocks tainted external/tool-output/memory-recall origins.
    - `*current-originating-signal*` dynamic variable propagates taint through orchestration chain. Set during `orchestrate-signal`, nil during `orchestrate-prompt` (owner trust).
    - Split dispatch: `orchestrate-once` dispatches to `orchestrate-signal` (external, never tool-parses payload) or `orchestrate-prompt` (internal, may contain tool commands).
    - All 12+ `read-from-string` calls on external data replaced with `%safe-parse-number` and `%safe-parse-policy-value` (no Lisp reader macros).
    - `*read-eval*` bound to nil at every remaining `read-from-string` site.
    - `%invariant-guard` — hardcoded non-configurable safety limits (vault min_harmony >= 0.30, dissonance-weight >= 0.05).
  - **Adaptive Shell (harmonic, self-tuning)**:
    - Gateway `Signal` struct carries `dissonance: f64` from injection scanning at parse time.
    - `signal-integrity` crate — shared injection detection + dissonance scoring (extended patterns: social engineering, Lisp reader macros, Harmonia-specific tool injection).
    - `route_allowed_with_context` in harmonic-matrix — security-aware routing with `security_weight` and `dissonance` parameters.
    - `:security-audit` phase added to harmonic state machine (observe injection counts, update posture, auto-adjust noise floors).
    - `*security-posture*` tracking (`:nominal`/`:elevated`/`:alert`).
  - **Boundary Wrapping**: External data wrapped with `=== EXTERNAL DATA [...] ===` markers in prompt assembly, memory recall, and search tool results (search-exa, search-brave).
  - **Matrix Hardening**: Raised min_harmony on privileged edges — vault `0.10→0.70`, harmonic-matrix `0.10→0.60`, git-ops `0.20→0.55`.
  - **Tailnet HMAC Auth**: `MeshMessage` carries `timestamp_ms` + `hmac` (HMAC-SHA256). 5-minute replay window. Shared secret from env var.
  - **MQTT Fingerprint Validation**: `validate_agent_fingerprint` compares `agent_fp` against vault-stored expected fingerprint.
  - **Vault Encryption at Rest**: Values encrypted with AES-256-GCM, rooted in wallet slot family `vault` (legacy-compatible with `harmonia-vault`) first; explicit `HARMONIA_VAULT_MASTER_KEY` is fallback-only. Component-scoped read policy enabled.
  - **Admin Intent Crate**: Ed25519 signature verification for privileged mutations (`lib/core/admin-intent`).
  - **Config**: `:security` section added to `config/harmony-policy.sexp` with privileged-ops list, dissonance-weight, admin-intent-required-for.
- New Crates: `lib/core/signal-integrity`, `lib/core/admin-intent`.
- Risk Notes: Typed signal dispatch is backward compatible — string prompts still handled by `orchestrate-prompt`. `#[serde(default)]` on new MeshMessage fields preserves tailnet backward compatibility.
- Rollback Plan: Revert `orchestrate-once` to string-only dispatch; remove policy gate calls; restore format-string gateway-inbound prompts.

## Next Entry Template

Use this structure for the next evolution record:

```md
## vN — YYYY-MM-DD

- Target:
- Motivation:
- Law/Principle Applied:
- Score Before:
- Score After:
- Risk Notes:
- Rollback Plan:
```
