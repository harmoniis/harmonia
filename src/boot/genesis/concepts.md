# Core Concepts

This document captures the conceptual model that should remain stable across implementation changes.

## Harmony As Operational Discipline

Harmony in Harmonia is not aesthetic language only. It is operationalized as:

- high completion with low failure,
- low noise in routing and memory,
- and composable structures that can be validated and evolved.

## Compression As Intelligence Pressure

The system prefers compressed representations that preserve utility.

Examples:

- daily interactions compressed into reusable skills,
- codemode pipelines collapsing many relay turns into one deterministic tool chain,
- policy in data files instead of deeply nested hardcoded branches.

## Attractor-Seeking Runtime

Harmonic planning uses attractor-inspired dynamics (logistic, lambdoma, lorenz) to steer rewrite timing.

Goal: avoid both chaotic rewrites and static stagnation.

## Genomic vs Epigenetic Layers

- Genomic layer: source and configuration structure.
- Epigenetic layer: runtime weights, scores, and mutable policy state.

Healthy evolution keeps these layers synchronized without collapsing them.

## Four-Pillar Capability Model

Rust capability surface is intentionally partitioned:

- core,
- backends,
- tools,
- frontends.

This keeps expansion predictable and boundaries clear.

## Boundary-First Safety

Three boundaries are central:

- vault boundary for secrets,
- matrix boundary for route permissions,
- gateway boundary for channel ingress/egress.

Any evolution that weakens one boundary increases systemic risk.

## Capabilities Over Names

Frontend behavior is driven by declared capabilities, not identity checks. A frontend declares what it can do (`:a2ui "1.0"`, `:push "t"`) in its baseband config. The conductor inspects signal capabilities, never frontend names. This keeps the architecture open for any future frontend to gain rich UI or push support by simply declaring the capability.

## Signal Enrichment

Gateway signals carry two enrichment layers beyond payload:

- **Capabilities** (static, from config): what the frontend can do.
- **Metadata** (dynamic, per-message): what the specific device/session provides.

This separation keeps the agent informed without coupling signal processing to specific frontend implementations.

## Security Kernel

The security kernel is a deterministic, non-bypassable layer that protects privileged operations:

- **Typed signals**: External data enters as `harmonia-signal` structs with security labels and taint tags, never as raw executable strings.
- **Policy gate**: Binary allow/deny gate for privileged operations (vault, matrix, config mutations). Checks taint chain and security label — not harmonic scores.
- **Taint propagation**: `*current-originating-signal*` tracks the signal that initiated each reasoning chain. Even if the LLM is tricked by prompt injection, the policy gate sees the tainted origin.
- **Invariant guards**: Hardcoded safety limits that cannot be weakened by configuration or admin intent (e.g., vault min_harmony >= 0.30).

Key principle: **LLM output is a proposal, not a command.** For non-privileged operations, proposals flow through harmonic routing. For privileged operations, proposals must pass the deterministic policy gate.

## Adaptive Security Shell

Complementing the hard security kernel, the adaptive shell provides defense-in-depth:

- **Dissonance scoring**: Injection pattern detection at gateway ingestion, producing a 0.0-1.0 dissonance score per signal.
- **Security-aware routing**: Harmonic matrix attenuates signals with high dissonance or low security weight.
- **Security posture tracking**: Autonomous monitoring of injection rates per frontend, with auto-adjustment of noise floors.
- **Boundary wrapping**: External data in prompts, memory recalls, and tool results is wrapped with security markers to resist prompt injection.

The kernel stops exploits structurally. The shell detects and attenuates anomalies adaptively.

## Evolution With Rollback

Every meaningful rewrite path must preserve rollback viability.

Improvement without rollback is treated as unsafe mutation, not evolution.
