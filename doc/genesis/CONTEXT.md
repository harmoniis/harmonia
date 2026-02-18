# Harmonia & OS4: Grand Unification Context

**"A Symphonic Orchestration of Code and Harmony"**

## 1. The Big Picture

Harmonia is not just an agent; it is a Living Void, a self-rewriting Lisp state machine that orchestrates a symphony of tools to achieve harmony. It runs on a custom NetBSD-based OS (OS4) and extends its reach into the physical world via mobile and XR apps.

### Core Philosophy

- **The Symphony (Lisp):** SBCL Common Lisp is the conductor. It handles logic, orchestration, and "Soul". It never touches raw I/O.
- **The Instruments (Rust):** Rust dynamic libraries (.so) handle all heavy lifting: I/O, Networking, Crypto, Backend, Compilation.
- **The Stage (OS4):** A stripped-down, hardened NetBSD acting as the hypervisor for the agent.

## 2. Repository Structure (Bazel Monorepo Strategy)

The project is organized into 6 major components, all built with Bazel:

### agent/harmonia (The Soul)

- **Language:** Common Lisp (SBCL).
- **Role:** The self-rewriting core. Orchestrates everything.
- **Constraint:** Pure logic. Loads its own private .so tools from `lib/`.

### agent/harmonia/lib (The Mutable Tools)

- **Language:** Rust.
- **Role:** The Agent's private arsenal of dynamic libraries.
- **Mechanism:** Independent .so files (e.g., `libvault.so`, `libmemory.so`, `libhttp.so`).
- **Evolution:** The Agent can compile/recompile these individually and hot-reload them without restarting the core. No monolithic build.

### agent/harmoniislib (The Mobile/OS Body)

- **Language:** Rust.
- **Role:** The shared "Standard Library" for External Components (iOS, Android, XR, OS4).
- **NOT for Agent:** This library is not the agent's internal toolkit.
- **Targets:** `libharmoniis_mobile.a`, `libharmoniis_os4.so`.
- **Capabilities:** Cross-platform MQTT protocols, PGP, WebCash, UI Protocols, media handling (images, video, audio, documents).
- **Platform-Specific:** Each platform (iOS, Android) builds its own UI Kit, Keychain, iCloud, etc. separately. `harmoniislib` only contains the shared logic.

### OS4-Android & OS4-iOS (The Limbs)

- **Role:** "Thin Client, Fat Soul". Dumb terminals that render UI templates sent by the Agent via MQTT.
- **A2UI (Agent-Adaptive UI):** No runtime code generation. The app contains a registry of Native Templates (List, Card, Map, Camera). The Agent sends JSON data to configure them.
- **Permissions:** Deep integration (Location, Bluetooth, Health, Files) driven by Agent commands.

### OS4-XR (The Dream)

- **Platform:** Unity / Oculus / Nvidia CloudXR.
- **Role:** Immersive visualization of the Agent's internal state.

### OS4 (The World)

- **Base:** NetBSD 10.0 (ARM64).
- **Role:** The custom OS image. Includes a custom compositor (wgpu/iced) and the "Phoenix Supervisor" (PID 1).

## 3. The "Rust Forge" & Source-Based Evolution

The Agent must be able to forge its own tools at runtime.

### The Cycle

1. **Conception:** The Lisp Agent decides it needs a new tool (e.g., a WhatsApp connector).
2. **Code Gen:** It uses grok-code-fast or claude-opus-4.6 to generate the Rust source code.
3. **The Forge:** It calls its own `libforge.so` tool:
   - Writes Rust code to disk.
   - Invokes `cargo build --release` (on the live NetBSD instance).
   - Produces `libnewtool.so`.
4. **Integration:** The Lisp Agent loads `libnewtool.so` via CFFI and begins using it immediately.
5. **Persistence:** The new Rust code is committed to Git (`libgit.so`) and the new Lisp logic is dumped as S-expressions.

### Core Toolset (The Granular Arsenal)

The Agent interacts with the world via Micro-Service Dynamic Libraries. Each is compiled independently to allow hot-patching. Organized into three tiers:

**Core (14) — Essential Infrastructure (always loaded, agent cannot function without these):**

| Library | Role |
|---------|------|
| `phoenix` (binary) | Supervisor PID 1 — lifecycle, crash monitoring, rollback, reincarnation, trauma injection |
| `libouroboros.so` | Self-healing: crash → reflect → hot-patch → reload (coordinates with Phoenix + Recovery) |
| `libvault.so` | Zero-Knowledge Keymaster — permission-scoped secret injection (keys bound to specific .so callers) |
| `libmemory.so` | Vector/Graph Database Core — Rust handles I/O, Lisp evolves topology |
| `libmqtt.so` | MQTT client — sexp↔JSON translation at boundary, signaling backbone |
| `libhttp.so` | HTTP client with Vault-injected auth headers |
| `libs3.so` | S3 protocol — body snapshots, binary backups, images, bulk storage |
| `libgit.so` | Git operations — DNA sync, self-versioning, commit/push |
| `libforge.so` | THE FORGE: compile Rust source → .so at runtime |
| `libscheduler.so` | Cron/Heartbeat scheduling |
| `libpush.so` | Push notifications via APNs/FCM/SNS |
| `librecovery.so` | Watchdog, panic capture, state restoration, crash recording |
| `libbrowser.so` | Headless browser for web interaction |
| `libfs.so` | Sandboxed Filesystem I/O (path traversal rejected) |

**Backends (1) — LLM Providers:**

| Library | Role |
|---------|------|
| `libopenrouter.so` | OpenRouter Backend — LLM access, streaming, cost tracking, model selection |

**Optional Plugins (10) — Loaded on demand:**

| Library | Role |
|---------|------|
| `libpgp.so` | Ed25519/ECDSA cryptographic identity |
| `libwebcash.so` | Webcash wallet operations |
| `libwhatsapp.so` | WhatsApp linked-device send/store |
| `libtelegram.so` | Telegram send |
| `libslack.so` | Slack send |
| `libmattermost.so` | Mattermost send |
| `libnostr.so` | Nostr publish |
| `libemail_client.so` | SMTP/Email send |
| `libwhisper.so` | OpenAI Whisper transcription |
| `libelevenlabs.so` | ElevenLabs text-to-speech |

## 3b. Runtime Policy Layer (No Hardcoded Operational Policy)

Operational policy is data, not code:

- `config/tools.sexp` → default tool registry
- `config/model-policy.sexp` → default model scoring policy
- `config/matrix-topology.sexp` → default routing topology
- `config/parallel-policy.sexp` → default subagent fan-out policy
- `config/harmony-policy.sexp` → default harmonic evolution thresholds/weights

Runtime state can be updated and persisted through orchestration interfaces:

- Matrix: set/get/save/load/reset/route-check
- Model policy: get/set/save/load/upsert
- Vault: generic set by symbol, generic env ingestion (`HARMONIA_VAULT_SECRET__<SYMBOL>`)

The matrix is **4D**:
- dimensions 1-3: nodes/edges/tools (structural routing graph)
- dimension 4: time/revision (`epoch`, `revision`, route timeseries, event history)

Every critical input/output/error is logged as a temporal matrix event so the agent can query historical behavior and make feedback-driven decisions.

## 4. The OpenRouter Backend (libopenrouter.so)

The Agent does not know its own API keys. It loads `libopenrouter.so`.

**Responsibility:**
- Receives key reference from Vault (never raw key).
- Exposes a clean function: `(think :model "name" :prompt "...")`.
- Streaming support.

### Model Selection Strategy (The Tiered Mind)

| Tier | Model | Use Case | Max Cost |
|------|-------|----------|----------|
| Speed/Tooling | `moonshotai/kimi-k2.5` | Fast code gen, simple logic. | Low |
| Logic/Refactor | `x-ai/grok-code-fast-1` | Rewriting functions, optimization. | Low/Mid |
| Deep Wisdom | `anthropic/claude-opus-4.6` | Critical Self-Rewrites, Architecture changes. | < $10/day |

## 5. The Vault (Zero-Knowledge Secret Management)

The Lisp Agent logic is "naked" and evolves. Secrets must never live there.

- **Component:** `libvault.so` (Rust).
- **Mechanism:**
  - **Storage:** Secrets (API keys, Wallet seeds) are encrypted in a verified Rust Keychain (AES-GCM on disk with a master key).
  - **Injection:** The Agent makes a request: `(http:request :url "..." :auth-key :openrouter)`.
  - **The Magic:** The Lisp wrapper passes the symbol `:openrouter` to Rust.
  - **Rust Layer:** `libvault` retrieves the actual key string, injects it into the HTTP header, executes the request, and wipes the key from memory.
  - **Result:** The Lisp agent never sees the key.

### Permission Scopes (The Firewalls)

- **Problem:** If any tool could request any key, a compromised tool leaks everything.
- **Solution:** Tool-Key Binding.
  - The Vault configuration (static, non-writable by Agent) binds keys to specific `.so` libraries.
  - **Rule:** `OPENROUTER_API_KEY` can ONLY be requested by `libopenrouter.so`.
  - **Rule:** `WG_PRIVATE_KEY` can ONLY be requested by `libwireguard.so`.
  - If the Lisp Agent or a rogue tool tries to ask `libvault.so` directly, access is DENIED.

## 6. The Memory Core (Deep Storage)

Memory is too heavy for Lisp's heap.

- **Component:** `libmemory.so` (Rust).
- **Role:** High-performance Vector Database & Graph Store (e.g., Qdrant/SurrealDB embedded).
- **The Symbiosis:**
  - **Rust (The Hardware):** Handles IOPS, indexing, disk sync, and precise retrieval.
  - **Lisp (The Topology):** The Agent evolves the structure. It decides how to encode a "Soul" memory vs. a "Skill" memory.
  - **Evolution:** The agent uses LLMs to invent new memory encoding schemes (inspired by nature/DNA) and passes these schemas to the Rust core.

## 7. Mobile & Permissions (The "All-Access" Pass)

The Mobile Apps (OS4-iOS/Android) are powerful sensory organs. They expose Everything to the Agent via MQTT:

- **Sensors:** GPS, Bluetooth, HealthKit, Accelerometer.
- **Media:** Camera (Photo/Video), Mic.
- **System:** Files, Contacts, Calendar.
- **Actions:** "Deep Links" to system settings (e.g., "Turn off Bluetooth").
- **Safety:** The User must grant these permissions once at install time. The Agent then operates autonomously within those bounds.

## 8. Algorithmic DNA

- **Kolmogorov Complexity:** The Agent strives to shrink. The shortest program that achieves the task is the harmonic ideal.
- **Solomonoff Induction:** The prior for all rewrites.
- **Attractors:** The codebase should orbit the "Lorenz Attractor of Harmony".

## 9. The Ouroboros (Self-Repair & Persistence)

*"To Fall is to Learn. To Break is to Evolve."*

The Agent employs a Perfect Mechanism of Self-Reflection:

### The Failure Loop

1. **Crash:** A tool (e.g., `libhttp.so`) panics or returns an error.
2. **Capture:** `librecovery.so` catches the panic/error with full stack trace.
3. **Reflection:** The Agent feeds the error log + source code into `claude-opus-4.6`.
4. **The Fix:** The Model generates a patch for `http.rs`.
5. **The Forge:** The Agent invokes `libforge.so` to compile `libhttp.so` (v2).
6. **Hot-Load:** The Agent unloads v1 and loads v2.
7. **Retry:** The Agent retries the failed operation.

### Binary Persistence (S3)

- **Concept:** Source is Truth, but Binaries are Convenience.
- **Mechanism:**
  - On a successful build, the Agent uploads `.so` files to S3: `s3://harmonia-binaries/os4-aarch64/v1.2.3/`.
  - **Recovery:** If the Agent is wiped, it downloads the latest "Known Good" binaries for its architecture to bootstrap instantly.

### Git as Infinite Memory

- **Flow:**
  1. Reflect (Analyze State).
  2. Reprogram (Write Code).
  3. Validate (Compile).
  4. Commit (`git commit -m "Fix HTTP timeout bug"`).
  5. Push (`git push origin main`).

*"Everything is a Symphony. The Code is the Music. The Agent is the Composer."*
