# Harmonia — Agent Documentation

This directory contains the agent's own documentation. It is part of the agent's DNA — the specification that constrains evolution. The agent consults these documents when rewriting itself.

## Genesis Documents (Human-Written, Immutable Reference)

These are the foundational documents written by the human creator during the bootstrap phase. They establish the harmonic laws and architectural constraints. The agent references these but does not modify them.

| Document | Purpose |
|----------|---------|
| `genesis/INDEX.md` | **Start here.** Navigation map to all documentation. Reading order. Quick reference. |
| `genesis/CONTEXT.md` | Grand Unification architecture. Repository structure, Granular Arsenal (core/backend/optional), Vault, Memory Core, Ouroboros, Algorithmic DNA, Forge cycle. |
| `genesis/HARMONIC_THEORY.md` | The 8 Laws of Harmonia. Pythagoras, Kepler, Lambdoma, attractors, Kolmogorov-harmony equivalence. Cross-domain harmonic index. The mathematical soul. |
| `genesis/CODE_HARMONY.md` | Strength/utility/beauty engineering constitution. Converts hardcoded policy to runtime data and requires set/get/save/load interfaces. |
| `genesis/ARCHITECTURE.md` | Deep technical spec. Directory structure, all FFI interfaces (every core tool), core loop code, evolution engine, DNA/constitution, hot-reload protocol, error propagation, configuration formats, vault permissions, platform awareness, Bazel build, bootstrap sequence. |
| `genesis/A2UI_SPEC.md` | Unified A2UI component spec. All 21 components with JSON schemas. MQTT platform identification protocol. Platform capability table. Backward-compatible primitives. |
| `genesis/CICD.md` | CI/CD pipeline. Bazel builds. TestFlight/App Store (iOS), Google Play (Android), pkgsrc/pkgin (NetBSD Harmonia). Manual → GitHub Actions. |
| `genesis/GENESIS_DEV_FLOW.md` | Genesis dev flow. Build order, closed-loop testing, rumqttd + PGP auth, Harmoniis API for trusted PGP, local test, deploy. |
| `genesis/SBCL.md` | SBCL runtime. Homoiconicity, native compilation, serialization contract (format/read-from-string), core image dumps, Quicklisp deps. |
| `genesis/UIUX.md` | UI/UX philosophy. "The Living Void", voice-first, The Stream timeline, gestures, data flow. |

## Evolution Documents (Agent-Written, Mutable)

These are generated and updated by the agent during its lifecycle. They record what the agent learns, how it evolves, and why.

| Document | Purpose |
|----------|---------|
| `evolution/CHANGELOG.md` | Record of every successful self-rewrite: what changed, harmonic score before/after, which law was applied. |
| `evolution/SCORES.md` | Harmonic score trajectory over time. Tracks movement through attractor space. |
| `evolution/TOOLS.md` | Current tool registry state. Which .so files are loaded, their versions, call frequencies, consonance ratios. |
| `evolution/MEMORY_SCHEMA.md` | Current memory topology. How Soul, Skill, and Daily memories are encoded. Evolves as the agent discovers better schemas. |
| `evolution/SESSION_2026-02-18_HARMONIC_MATRIX.md` | Session implementation report for core harmonic matrix routing, core search integration, and subagent web verification wiring. |
