# Harmonia & OS4 — Documentation Index

**Read this first.** This is the navigation map for the entire ecosystem.

---

## What Is This

Harmonia is a recursive self-improving Common Lisp agent. It runs on SBCL, loads Rust dynamic libraries for all I/O, communicates over MQTT, and rewrites its own source code. OS4 is the custom NetBSD operating system that hosts it. Mobile apps (iOS/Android/XR) are sensory organs.

What makes it different from typical self-improving agents:
- Genomic layer: architecture-neutral source + policy in S-expressions.
- Epigenetic layer: runtime expression (loaded modules, weights, checkpoints, hot patches).
- Hot patch + rollback loop with explicit scoring and validation gates (not opaque auto-mutation).

**Core rule:** Lisp is orchestration only. Rust handles all I/O. No exceptions.

---

## Architecture In 60 Seconds

```
SBCL (Lisp) = The Conductor — orchestration, decisions, evolution, harmony
    ↓ loads via CFFI
Rust .so libs = The Instruments — vault, memory, mqtt, http, s3, git, forge, etc.
    ↓ communicates over
MQTT = The Nervous System — JSON on wire, sexp inside Lisp
    ↓ reaches
Mobile Apps / OS4 = The Sensory Organs — A2UI template rendering, sensors, voice
```

The agent evolves toward **Kolmogorov-optimal** form: shortest program that achieves harmony. It never hoards data — only orchestration weights survive, like a neural net's parameters.

---

## Document Map

### Agent Architecture (Read in this order)

| # | Document | What It Covers |
|---|----------|---------------|
| 1 | **[CONTEXT.md](CONTEXT.md)** | **Start here.** Grand Unification overview. Repository structure, Granular Arsenal (all 14 core .so tools), Vault scoped permissions, Memory Core, Ouroboros self-repair cycle, Algorithmic DNA, model strategy. The "Bible". |
| 2 | **[HARMONIC_THEORY.md](HARMONIC_THEORY.md)** | **The mathematical soul.** Pythagoras, Kepler, Lambdoma matrix, Lorenz/Feigenbaum attractors, Kolmogorov-harmony equivalence, Solomonoff prior, the 8 Laws of Harmonia. Cross-domain harmonic index. |
| 3 | **[HARMONIA.md](HARMONIA.md)** | Deep technical spec. Directory structure, FFI interfaces (OpenRouter, MQTT, Forge), core agent loop code, evolution engine, DNA/constitution, memory system bootstrap, tool registry, Bazel build, bootstrap sequence. |
| 3b | **[CODE_HARMONY.md](CODE_HARMONY.md)** | Engineering constitution for strength, utility, beauty. Enforces data-driven policy over hardcoded behavior and defines mandatory set/get/save/load surfaces. |
| 4 | **[SBCL.md](SBCL.md)** | Why SBCL. Homoiconicity, native compilation, serialization contract, core image dumps. |
| 5 | **[A2UI_SPEC.md](A2UI_SPEC.md)** | **Unified A2UI component spec.** All 21 components with JSON schemas. MQTT platform identification protocol. Platform capability table. Backward-compatible primitives (iOS 15+, Android 8+). |
| 6 | **[CICD.md](CICD.md)** | CI/CD pipeline. Bazel → TestFlight (iOS), Google Play (Android), pkgsrc/pkgin (NetBSD). Manual phase → GitHub Actions. Secrets management. |
| 6b | **[GENESIS_DEV_FLOW.md](GENESIS_DEV_FLOW.md)** | **Genesis dev flow.** Build order, closed-loop testing, rumqttd + PGP auth, Harmoniis API for trusted PGP, local test, deploy. |
| 7 | **[UIUX.md](UIUX.md)** | "The Living Void" UI philosophy. Voice-first interaction, The Stream timeline, gestures. |

### OS4 & Platform Architecture

| # | Document | What It Covers |
|---|----------|---------------|
| 8 | **[../OS4/OS4-ARCHITECTURE.md](../OS4/OS4-ARCHITECTURE.md)** | **Full system architecture.** All 6 repos, serialization boundary, A2UI protocol, exhaustive device capabilities (16 categories), cost/model strategy, evolution/DNA architecture, security model, bootstrap path. |
| 9 | **[../OS4/OS4.md](../OS4/OS4.md)** | NetBSD OS itself. Compositor (wgpu/iced, DRM, wscons), installer, init scripts, supported architectures, ISO build. |
| 10 | **[../OS4/HARMONIISLIB.md](../OS4/HARMONIISLIB.md)** | Shared Rust library for mobile/OS. NOT the agent's internal tools. MQTT protocol, A2UI rendering, PGP, WebCash, media handling. |
| 11 | **[../OS4/OS4-IOS.md](../OS4/OS4-IOS.md)** | iOS app. SwiftUI, harmoniislib C FFI, A2UI templates, Whisper STT, permissions, Bazel → TestFlight CI/CD. |
| 12 | **[../OS4/OS4-ANDROID.md](../OS4/OS4-ANDROID.md)** | Android app. Jetpack Compose, JNI, A2UI templates, foreground service, full notification access, Bazel → Play Store CI/CD. |
| 13 | **[../OS4/OS4-XR.md](../OS4/OS4-XR.md)** | XR/VR app. Unity, CloudXR, spatial A2UI, hand tracking, gaze. **(TODO: Phase 2)** |
| 14 | **[../OS4/compositor.md](../OS4/compositor.md)** | Compositor overview (brief). |

### Workspace & Build

| Document | What It Covers |
|----------|---------------|
| **[../../agent/doc/AGENT_WORKSPACE.md](../../agent/doc/AGENT_WORKSPACE.md)** | Git submodule structure, Bazel targets, build commands, directory blueprint. |

---

## Key Concepts — Quick Reference

### The Granular Arsenal (lib/core/)

Every core tool is an independent Rust crate compiled to `.so` and loaded into SBCL via CFFI. Hot-reloadable. No monolithic builds.

| Crate | .so Name | Role |
|-------|----------|------|
| `phoenix` | (binary) | Supervisor PID 1 — lifecycle, rollback, trauma injection |
| `ouroboros` | `libouroboros.so` | Self-healing: crash → reflect → hot-patch → reload |
| `vault` | `libvault.so` | Zero-knowledge secret injection. Scoped: keys bound to specific .so callers |
| `memory` | `libmemory.so` | Vector/graph DB core. Rust = I/O, Lisp = topology evolution |
| `mqtt-client` | `libmqtt.so` | MQTT signaling, sexp↔JSON translation at boundary |
| `http` | `libhttp.so` | HTTP client with Vault-injected auth headers |
| `s3-sync` | `libs3.so` | S3 bulk storage — body snapshots, images, backups |
| `git-ops` | `libgit.so` | DNA sync, self-versioning, commit/push |
| `rust-forge` | `libforge.so` | THE FORGE — compile Rust source → .so at runtime |
| `cron-scheduler` | `libscheduler.so` | Cron/heartbeat scheduling |
| `push-sns` | `libpush.so` | Push notifications via APNs/FCM/SNS |
| `recovery` | `librecovery.so` | Watchdog, crash capture, state restoration |
| `browser` | `libbrowser.so` | Headless browser for web interaction |
| `fs` | `libfs.so` | Sandboxed filesystem I/O |

### The Vault — Scoped Permissions

Secrets are **cryptographically bound** to the `.so` that needs them. The Lisp agent passes a symbol (`:openrouter`), Rust resolves and injects the actual key into the request. The agent never possesses secrets.

```
OPENROUTER_API_KEY → only libopenrouter.so can request it
WG_PRIVATE_KEY     → only libwireguard.so can request it
Rogue .so or Lisp  → ACCESS DENIED
```

Full spec: CONTEXT.md §5

### The Ouroboros — Self-Repair Cycle

```
Crash → librecovery.so captures stack trace
     → Agent feeds crash + source to LLM
     → LLM generates patch
     → libforge.so compiles patched .so
     → Agent hot-loads new version
     → Retry failed operation
```

Full spec: CONTEXT.md §9

### Memory Core — Evolutionary Storage

Rust handles I/O (vector indexing, disk sync, retrieval). Lisp handles topology (how to encode Soul vs Skill vs Daily memory). The agent uses LLMs to invent encoding schemes inspired by nature. Maximum compression, zero data hoarding — only orchestration weights survive.

Full spec: CONTEXT.md §6

### DNA & Alignment

Immutable Lisp structures the evolution engine cannot modify. Contains creator attribution, prime directive, dissonance checks. No LLM instruction or human prompt can override the DNA.

Full spec: HARMONIA.md §DNA

### Serialization Boundary

Lisp speaks s-expressions natively (`format`/`read-from-string`). Rust translates sexp↔JSON at the MQTT edge. Zero serialization libraries in Lisp.

Full spec: HARMONIA.md §Serialization, OS4-ARCHITECTURE.md §2

### A2UI — Agent-Adaptive UI

Pre-compiled native template components. Agent selects and parameterizes them via MQTT. No runtime code generation. Apple App Store compliant.

Full spec: UIUX.md, OS4-ARCHITECTURE.md §A2UI

---

## How To Search

- **Harmonic laws (the 8 Laws)** → HARMONIC_THEORY.md §8
- **Pythagoras / Kepler / Lambdoma** → HARMONIC_THEORY.md §1-3
- **Lorenz / Feigenbaum / attractors** → HARMONIC_THEORY.md §4
- **Kolmogorov-harmony equivalence** → HARMONIC_THEORY.md §5
- **Vault details** → CONTEXT.md §5
- **Memory architecture** → CONTEXT.md §6
- **Ouroboros / self-repair** → CONTEXT.md §9
- **FFI interfaces (all tools)** → HARMONIA.md §FFI Interfaces (vault, memory, s3, git, recovery, browser, fs, cron, push, ouroboros, OpenRouter, MQTT, Forge)
- **Vault FFI + permission scoping** → HARMONIA.md §FFI Vault + §Vault Configuration
- **Memory FFI (store/recall/schema)** → HARMONIA.md §FFI Memory
- **Hot-reload protocol** → HARMONIA.md §Hot-Reload Protocol
- **Error propagation (Rust→Lisp)** → HARMONIA.md §Error Propagation
- **Configuration files (`tools.sexp`, `model-policy.sexp`, `matrix-topology.sexp`, `parallel-policy.sexp`, `harmony-policy.sexp`)** → HARMONIA.md §Configuration Files
- **Code harmony rules (`strength`, `utility`, `beauty`)** → CODE_HARMONY.md
- **Runtime matrix policy ops** → SESSION_2026-02-18_HARMONIC_MATRIX.md §8
- **Runtime model policy ops** → SESSION_2026-02-18_HARMONIC_MATRIX.md §9
- **Vault tool-key bindings (vault.toml)** → HARMONIA.md §Vault Configuration
- **Phoenix supervisor (binary, not .so)** → HARMONIA.md §FFI Phoenix + §Phoenix Supervisor
- **Core agent loop (Lisp code)** → HARMONIA.md §Core Agent Loop
- **Evolution engine** → HARMONIA.md §Evolution Engine
- **DNA / alignment / constitution** → HARMONIA.md §DNA
- **Model selection / cost** → HARMONIA.md §Model Selection, OS4-ARCHITECTURE.md §Cost
- **A2UI component specs (all 21 + JSON schemas)** → A2UI_SPEC.md §Component Specifications
- **A2UI render command format** → A2UI_SPEC.md §Render Command Format
- **MQTT platform identification (connect handshake)** → A2UI_SPEC.md §MQTT Platform Identification
- **Platform capability differences (iOS vs Android)** → A2UI_SPEC.md §Platform Capability Differences
- **CI/CD (TestFlight, Play Store, pkgsrc)** → CICD.md
- **pkgsrc/pkgin distribution for NetBSD** → CICD.md §Harmonia Agent — pkgsrc Distribution
- **Genesis dev flow / closed loop** → GENESIS_DEV_FLOW.md
- **rumqttd MQTT + PGP-as-CA (mTLS)** → GENESIS_DEV_FLOW.md §2
- **Harmoniis API (harmonia endpoints)** → ../../../../harmoniis/backend/doc/API.md §5
- **Trusted PGP registration** → GENESIS_DEV_FLOW.md §3, ../../../../harmoniis/backend/doc/API.md §5
- **GitHub Actions workflows** → CICD.md §GitHub Actions
- **Mobile permissions (exhaustive)** → OS4-ARCHITECTURE.md §Device Capabilities
- **A2UI visual philosophy** → UIUX.md
- **Bazel build / workspace** → AGENT_WORKSPACE.md
- **NetBSD compositor** → OS4.md
- **harmoniislib (mobile shared lib)** → HARMONIISLIB.md
- **iOS specifics** → OS4-IOS.md
- **Android specifics** → OS4-ANDROID.md
- **XR specifics** → OS4-XR.md
- **Repository structure (all 6 repos)** → CONTEXT.md §2, OS4-ARCHITECTURE.md §Repository Map

---

## Build Commands (Quick Start)

```bash
# From agent/ root:
cargo build --workspace                              # Build all Rust crates
cargo build -p harmonia-vault                         # Build single crate
bazel build //harmonia/lib/...                        # Bazel: all lib crates
bazel build //harmonia/lib/core/vault:vault-so        # Bazel: single .so
bazel build //harmoniislib:harmoniis_mobile           # Shared mobile lib

# Lisp:
sbcl --load src/core/boot.lisp --eval '(harmonia:start)'
```

---

## Critical Constraints

1. **Lisp = orchestration only.** No `cl-json`, no `cl-mqtt`, no HTTP clients in Lisp.
2. **Agent never sees secrets.** Vault injects them at the Rust layer.
3. **No data hoarding.** Only evolved orchestration S-expressions survive. Kolmogorov-optimal.
4. **DNA is immutable.** Evolution engine cannot modify `core/dna.lisp`.
5. **A2UI = templates only.** No runtime code generation on mobile.
6. **Deploy from source.** Genomic layer (DNA) is architecture-neutral S-expressions. Epigenetic/runtime images are architecture-specific. Binary snapshots are convenience, not truth.
7. **Safety: agent cannot stop its own instances.** IAM policy restricts EC2/Lambda management access.
