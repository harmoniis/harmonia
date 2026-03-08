# Concepts Glossary

## Architecture And Governance

- `Genesis`: foundational identity, constraints, and architecture intent.
- `Evolution`: controlled adaptation under genesis constraints.
- `Genomic Layer`: long-lived architecture/policy identity.
- `Epigenetic Layer`: mutable runtime expression and tuning.
- `Constitution`: non-negotiable rules for behavior and safety.
- `Vitruvian Triad`: strength, utility, beauty evaluation frame.

## Runtime And Orchestration

- `Conductor`: Lisp orchestration engine that routes prompts and tool ops.
- `Port`: Lisp capability boundary backed by Rust C-ABI.
- `Baseband`: unified signal ingress/egress processor (gateway).
- `Router`: LLM completion boundary used by orchestration.
- `Swarm`: parallel subagent system (API tier + tmux CLI tier).
- `Lineage`: VCS commit/push boundary for evolution provenance.
- `Matrix`: route-constraint and telemetry graph for allowed operations.

## Memory And Scoring

- `Memory Classes`: soul, skill, daily, tool (see memory schema docs).
- `Crystallization`: preserving high-signal memory before compression.
- `Token Harmony`: efficiency-aware extensions to harmonic scoring.
- `Attractor`: stable dynamics target for runtime evolution.
- `Lambdoma`: harmonic relation matrix used in theory/scoring framing.

## Recovery And Evolution

- `Ouroboros`: crash/patch self-repair subsystem.
- `Phoenix`: supervisor lifecycle and restart/rollout guard.
- `Artifact Rollout`: evolution mode where binary rollout is signaled.
- `Source Rewrite`: evolution mode where patch artifacts are generated/applied.
- `Rollback`: explicit recovery path after failed or harmful mutation.
- `Snapshot Versioning`: immutable `versions/vN` plus mutable `latest` model.

## Channels And UI

- `Frontend`: pluggable communication channel loaded through baseband.
- `Frontend Capabilities`: static feature declarations parsed from `:capabilities` in baseband config at registration. Attached to every signal from that frontend. Used for capabilities-driven dispatch (e.g., A2UI) without hardcoded frontend-name checks.
- `Signal Metadata`: dynamic per-message context emitted by a frontend as a third poll field. Contains device-specific info (platform, device ID, A2UI version, etc.).
- `Signal Enrichment`: the two-layer model where gateway signals carry both static capabilities (from config) and dynamic metadata (from frontend).
- `Tailnet`: tailscale mesh transport layer and inter-node channel substrate.
- `A2UI`: agent-adaptive UI template protocol. Dispatch is capabilities-driven — any frontend declaring `:a2ui` capability gets A2UI treatment.
- `A2UI Catalog`: canonical component definitions in `config/a2ui-catalog.sexp` (21 components). Lazily loaded by conductor and injected into LLM context for A2UI-capable signals.
- `A2UI Text Fallback`: automatic degradation of A2UI component payloads to plain text when sent to non-A2UI frontends.
- `Living Void`: UI/UX philosophy for voice-first adaptive interfaces.
- `Canonical Envelope`: shared message structure used across agent/platform clients.
- `Device Registry`: MQTT frontend's in-memory registry of connected devices with platform info, capabilities, push tokens, and online/offline state.
- `Offline Queue`: per-device message queue in MQTT frontend, flushed on reconnect with push notification for offline delivery.
- `Push Webhook`: HTTP POST-based push notification delivery via `lib/frontends/push` (rlib utility consumed by mqtt-client).

## Security

- `Security Kernel`: deterministic, non-bypassable layer protecting privileged operations via typed signals, policy gate, and taint propagation.
- `Adaptive Security Shell`: harmonic defense-in-depth layer using dissonance scoring, security-aware routing, and autonomous posture tracking.
- `Policy Gate`: binary allow/deny gate (`%policy-gate`) for 14 privileged operations. Checks originating signal's taint chain and security label. Not based on harmonic scores.
- `Taint Propagation`: tracking of signal origin through the orchestration chain via `*current-originating-signal*`. Taint labels: `:external`, `:tool-output`, `:memory-recall`, `:internal`.
- `Harmonia Signal`: typed struct replacing format-string prompts for external signals. Carries security-label, taint, dissonance, frontend, payload, capabilities, metadata.
- `Security Label`: trust classification of a signal's origin: `:owner`, `:authenticated`, `:anonymous`, `:untrusted`.
- `Dissonance Score`: 0.0-1.0 injection detection score computed at gateway signal parse time. High dissonance attenuates signal in security-aware routing.
- `Boundary Wrapping`: external data wrapped with `=== EXTERNAL DATA [...] ===` markers in prompts, memory recalls, and search results to resist prompt injection.
- `Invariant Guard`: hardcoded non-configurable safety limits (vault min_harmony >= 0.30, dissonance-weight >= 0.05) that cannot be weakened by any configuration or admin intent.
- `Security Posture`: system-wide security state (`:nominal`/`:elevated`/`:alert`) tracked by `:security-audit` phase in harmonic machine.
- `Signal Integrity`: shared crate (`lib/core/signal-integrity`) for injection pattern detection, dissonance scoring, and boundary wrapping.
- `Admin Intent`: Ed25519 signed authorization for privileged mutations. Owner's public key in vault, private key on owner's device.
- `Safe Parser`: `%safe-parse-number` and `%safe-parse-policy-value` — replacements for `read-from-string` that prevent Lisp reader macro attacks.
- `Confused Deputy`: attack where the LLM is tricked (via prompt injection) into proposing privileged actions on behalf of an untrusted signal. Mitigated by taint propagation + policy gate.
- `Vault Symbol`: symbolic handle to a secret value (not raw secret exposure).
- `Scoped Secret Access`: key access constrained to approved call paths.
- `Boundary-First Safety`: policy that sensitive operations are gated at explicit boundaries.
- `Vault Encryption at Rest`: AES-based encryption of stored vault secrets using master key derived from Harmoniis wallet `vault` slot (`HARMONIA_VAULT_MASTER_KEY` is fallback-only).
- `HMAC Authentication`: HMAC-SHA256 message authentication on tailnet mesh messages with 5-minute replay protection window.
- `Fingerprint Validation`: MQTT frontend validates `agent_fp` against vault-stored expected fingerprint; mismatches downgraded to untrusted.
