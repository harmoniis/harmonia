# harmonia — The Lisp Agent

## Repository: `$WORKSPACE/agent/harmonia`

Harmonia is the recursive self-improving Common Lisp agent. This is the soul of OS4. It runs on SBCL, loads Rust dynamic libraries for ALL tools, backends, and I/O, communicates over MQTT (via Rust `.so`), and rewrites its own source code to better discover harmonic truth.

**Cardinal rule: Lisp is orchestration only.** No tooling in Lisp. No `cl-mqtt`. No `cl-json`. No serialization libraries. No HTTP clients. The agent thinks, plans, composes, and orchestrates in Lisp. Everything that touches the outside world is a Rust dynamic library loaded via CFFI. All serialization and deserialization happens in Rust at the MQTT boundary — Lisp speaks s-expressions natively, and Rust translates at the edge.

---

## Directory Structure

```
harmonia/
├── BUILD.bazel                 # Bazel build rules
├── WORKSPACE.bazel             # Bazel workspace root
├── bazel/
│   ├── sbcl.bzl                # Custom Bazel rules for SBCL compilation
│   ├── rust_dynlib.bzl         # Rules for building Rust .so tools
│   └── defs.bzl                # Shared definitions
├── README.md                   # Agent documentation
├── src/                        # Lisp source
│   ├── core/
│   │   ├── boot.lisp           # Bootstrap: load CFFI, register tools, start loop
│   │   ├── loop.lisp           # Main agent loop: Read → Eval → Modify → Write → Validate
│   │   ├── eval.lisp           # S-expression evaluator and harmonic scorer
│   │   ├── state.lisp          # State machine runtime (persists across rewrites)
│   │   ├── rewrite.lisp        # Evolution engine: generate candidates, validate, apply
│   │   ├── rollback.lisp       # Safe rollback to previous version on failure
│   │   └── sanity.lisp         # Sanity checks: compile test, self-consistency, loop test
│   ├── memory/
│   │   ├── store.lisp          # Initial memory store (s-expression based, will evolve)
│   │   ├── recall.lisp         # Initial retrieval system (will evolve)
│   │   ├── compress.lisp       # Memory compression / forgetting (will evolve)
│   │   └── test-memory.lisp    # Memory benchmarking with synthetic data
│   ├── harmony/
│   │   ├── detector.lisp       # Pattern detection: resonances, constraints, coherence
│   │   ├── scorer.lisp         # Harmonic scoring function
│   │   ├── forbidden.lisp     # Forbidden state detection
│   │   └── domains.lisp        # Domain adapters (time series, text, graphs, etc.)
│   ├── orchestrator/
│   │   ├── conductor.lisp      # Tool orchestration: just-in-time tool invocation
│   │   ├── planner.lisp        # Multi-step plan generation
│   │   ├── cost.lisp           # Cost tracking and model selection optimizer
│   │   └── stream.lisp         # Response streaming handler
│   ├── tools/
│   │   ├── registry.lisp       # Tool registry: load, unload, discover tools
│   │   ├── cffi-bridge.lisp    # CFFI wrapper for loading Rust .so libraries
│   │   ├── tool-protocol.lisp  # Standard tool interface (call, result, error)
│   │   └── builtin/
│   │       └── time-tool.lisp  # Time utilities (get-universal-time wrappers only)
│   ├── backends/
│   │   ├── backend-protocol.lisp   # Backend interface: list-models, complete, stream, pricing
│   │   ├── backend-loader.lisp     # Load backend .so, extract function pointers via CFFI
│   │   └── model-selector.lisp     # Harmonic model selection: quality × speed ÷ cost
│   └── dna/
│       ├── snapshot.lisp       # Coordinate Body snapshot to S3 (via s3-sync .so)
│       ├── journal.lisp        # Journal modifications as s-expressions
│       ├── git-sync.lisp       # Coordinate DNA sync to Git (via git-ops .so)
│       └── merge.lisp          # Merge DNA from other agents
├── lib/                        # Rust source — decoupled crates, each buildable separately
│   ├── core/                   # Essential infrastructure (agent cannot function without these)
│   │   ├── phoenix/            # Supervisor (Rust binary, PID 1) — lifecycle, rollback, trauma
│   │   ├── ouroboros/          # Self-healing / reflection cycle (crash → reflect → hot-patch → reload)
│   │   ├── vault/              # Zero-knowledge secret injection (permission-scoped keys)
│   │   ├── memory/             # Rust storage backend for Lisp memory system
│   │   ├── mqtt-client/        # MQTT communication backbone (sexp↔JSON at boundary)
│   │   ├── http/               # HTTP client (.so for outbound requests)
│   │   ├── s3-sync/            # Body snapshot to S3
│   │   ├── git-ops/            # DNA sync to Git
│   │   ├── rust-forge/         # Self-extending tool system (compile .so at runtime)
│   │   ├── cron-scheduler/     # Cron / heartbeat scheduling
│   │   ├── push-sns/           # Push notifications (APNs/FCM via SNS)
│   │   ├── recovery/           # Watchdog & crash handling
│   │   ├── browser/            # Headless browser
│   │   └── fs/                 # Sandboxed filesystem I/O
│   ├── backends/               # LLM providers (.so for CFFI)
│   │   └── openrouter-backend/
│   └── tools/                  # Optional plugins (.so, loaded on demand)
│       ├── pgp-identity/       # Ed25519/ECDSA cryptographic identity
│       ├── webcash-wallet/     # Webcash operations
│       └── social/             # WhatsApp/Telegram/Discord clients
├── config/
│   ├── agent.sexp              # Initial agent configuration (s-expression)
│   ├── backends.sexp           # Backend configuration (names, .so paths)
│   └── tools.sexp              # Tool registry configuration
├── tests/
│   ├── test-boot.lisp
│   ├── test-rewrite.lisp
│   ├── test-memory.lisp
│   ├── test-harmony.lisp
│   └── test-tools.lisp
└── scripts/
    ├── bootstrap.sh            # First-time setup: install SBCL, Quicklisp, compile tools
    ├── run.sh                  # Start the agent (via Phoenix Supervisor)
    └── snapshot.sh             # Manual Body snapshot
```

---

## The Phoenix Supervisor (The Immortal Coil)

*   **Role:** An Erlang-style supervisor process (Rust binary, PID 1) that manages the Agent's lifecycle.
*   **Responsibility:**
    1.  Starts the Agent (SBCL process).
    2.  Monitors for crashes (exit code != 0, timeouts).
    3.  **Crash Analysis:** Captures stderr/crash dump.
    4.  **Rollback:**
        *   **Stage 1 (Code Logic):** `git revert HEAD` (undoes last evolution).
        *   **Stage 2 (System Failure):** **Local Backup + S3 Fallback.**
            *   **Try Local:** Checks `harmonia.core.bak` integrity (checksum). If valid, restores it.
            *   **Fallback:** If local is missing or corrupt, downloads previous snapshot from S3.
            *   Reboots the instance.
    5.  **Reincarnation:** Restarts the Agent.
    6.  **Trauma Injection:** Feeds the crash log back to the Agent as a high-priority input ("I died because of X"). Crash-as-Input pattern — every death teaches the agent what killed it.

---

## The Ouroboros (Self-Healing & Reflection)

*"To Fall is to Learn. To Break is to Evolve."*

The Ouroboros (`lib/core/ouroboros/`) implements the agent's self-repair mechanism. It coordinates with Phoenix (supervisor) and Recovery (watchdog) to turn failures into evolution.

### The Failure Loop

1. **Crash:** A tool (e.g., `libhttp.so`) panics or returns an error.
2. **Capture:** `librecovery.so` catches the panic/error with full stack trace.
3. **Reflection:** The Agent feeds the error log + relevant source code into the LLM (claude-opus-4.6 for critical failures).
4. **The Fix:** The LLM generates a patch.
5. **The Forge:** The Agent invokes `libforge.so` to compile the patched `.so` (e.g., `libhttp.so` v2).
6. **Hot-Load:** The Agent unloads v1, loads v2 via CFFI. No restart.
7. **Retry:** The Agent retries the failed operation with the patched tool.

### Binary Persistence (S3)

Source is Truth, but Binaries are Convenience:
- On successful build, the Agent uploads `.so` files to S3: `s3://harmonia-binaries/{arch}/v{version}/`.
- Recovery: If the Agent is wiped, it downloads the latest "Known Good" binaries for its architecture to bootstrap instantly.
- Architecture-specific: ARM agent downloads ARM binaries, x86 downloads x86.

---

## The Vault (Zero-Knowledge Secret Management)

*   **Component:** `lib/core/vault/` → `libvault.so` (Rust).
*   **Problem:** The Lisp Agent logic is "naked" and evolves. Secrets must never live there. If the agent's memory is dumped or inspected, no secrets should be found — only symbols.
*   **Mechanism:**
    *   **Storage:** Secrets (API keys, wallet seeds) are encrypted in a Rust keychain (AES-GCM on disk with a master key set at deploy time as env variable).
    *   **Injection:** The Agent makes a request: `(http:request :url "..." :auth-key :openrouter)`.
    *   **The Magic:** Lisp passes the symbol `:openrouter` to Rust. Rust resolves the actual key, injects it into the HTTP header, executes the request, and wipes the key from memory.
    *   **Result:** The Lisp agent never sees, possesses, or handles the actual secret.

### Permission Scopes (The Firewalls)

*   **Problem:** If any tool could request any key, a compromised tool leaks everything.
*   **Solution:** Tool-Key Binding. The Vault configuration (static, non-writable by Agent) cryptographically binds keys to specific `.so` libraries.
    *   `OPENROUTER_API_KEY` can **ONLY** be requested by `libopenrouter.so`.
    *   `WG_PRIVATE_KEY` can **ONLY** be requested by `libwireguard.so`.
    *   If the Lisp Agent or a rogue `.so` tries to ask `libvault.so` directly — **Access DENIED**.
    *   The Agent never possesses a secret, only the symbol referring to it.

---

## The Memory Core (Evolutionary Deep Storage)

*   **Component:** `lib/core/memory/` → `libmemory.so` (Rust).
*   **Problem:** Memory is too heavy for Lisp's heap. RAM is constrained (~1GB per agent on shared infrastructure).
*   **The Symbiosis:**
    *   **Rust (The Hardware):** High-performance vector database & graph store. Handles IOPS, indexing, disk sync, and precise retrieval.
    *   **Lisp (The Topology):** The Agent evolves the structure. It decides how to encode a "Soul" memory vs. a "Skill" memory vs. a "Daily" memory. Uses LLMs to invent new encoding schemes inspired by nature/DNA.
*   **Constraints:**
    *   No personal data hoarding — inefficient and forbidden.
    *   Maximum compression. The minimum-size representation is the best version (Kolmogorov complexity).
    *   Only orchestration weights survive, like a neural net's parameters.
    *   The structure itself evolves as the agent discovers better encoding schemes.

---

## Deployment Safety

### IAM Restrictions

The agent **cannot** access EC2/Lambda management APIs. A restricted IAM policy ensures the agent cannot stop, terminate, or modify its own infrastructure. The kill switch is external — only the human operator (via AWS console or a separate Lambda) can stop instances.

### API Key Management

OpenRouter API keys are set as **environment variables at deploy time** (AMI bake or Lambda configuration). They are never in source code, never in agent memory, never in Lisp. The Vault reads them from the environment at initialization and manages injection from there.

### Garbage Collection

SBCL's garbage collector must be tuned for the agent's memory constraints. The agent must not accumulate state — only evolved orchestration S-expressions persist. All task-specific data, conversation logs, and intermediate results are discarded after processing.

---

## FFI Interfaces — All Core Tools

Every Rust `.so` exposes `extern "C"` functions. Lisp calls them via CFFI. All string returns are s-expression formatted. Every library includes a `_free_string` function for memory management.

### Vault (`lib/core/vault/` → `libvault.so`)

```rust
/// Initialize vault. Reads master key from VAULT_MASTER_KEY env var.
/// Loads encrypted keychain from disk. Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn vault_init(keychain_path: *const c_char) -> i32;

/// Retrieve a secret by symbol name and inject it into a target operation.
/// `key_symbol`: The Lisp symbol name (e.g., "openrouter", "wg_private").
/// `caller_so_path`: Path of the .so making the request (for permission check).
/// Returns the secret as a C string, or NULL if denied.
/// SECURITY: Caller identity is verified by checking dladdr of the return address.
/// The secret must be wiped from memory after use.
#[no_mangle]
pub extern "C" fn vault_get_secret(
    key_symbol: *const c_char,
    caller_so_path: *const c_char,
) -> *mut c_char;

/// Inject a secret into an HTTP request header without exposing it to Lisp.
/// `key_symbol`: Secret name (e.g., "openrouter").
/// `header_name`: HTTP header to inject into (e.g., "Authorization").
/// `url`: Target URL.
/// `method`: HTTP method.
/// `body_sexp`: Request body as s-expression (or NULL).
/// Returns response as s-expression string.
/// The secret is fetched internally, injected, request executed, secret wiped.
#[no_mangle]
pub extern "C" fn vault_inject_request(
    key_symbol: *const c_char,
    header_name: *const c_char,
    url: *const c_char,
    method: *const c_char,
    body_sexp: *const c_char,
) -> *mut c_char;

/// Register a new secret (admin only, not callable by agent in production).
/// `key_symbol`: Name for the secret.
/// `value`: The actual secret value (encrypted at rest immediately).
/// `allowed_callers_sexp`: S-expression list of .so paths allowed to access this key.
///   ("libopenrouter.so" "libhttp.so")
#[no_mangle]
pub extern "C" fn vault_register_secret(
    key_symbol: *const c_char,
    value: *const c_char,
    allowed_callers_sexp: *const c_char,
) -> i32;

/// List all registered key symbols (not values). Returns s-expression list.
/// ("openrouter" "wg_private" "mqtt_credentials" ...)
#[no_mangle]
pub extern "C" fn vault_list_keys() -> *mut c_char;

/// Check if a specific .so is authorized for a key. Returns 1 (yes) or 0 (no).
#[no_mangle]
pub extern "C" fn vault_check_permission(
    key_symbol: *const c_char,
    so_path: *const c_char,
) -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn vault_free_string(ptr: *mut c_char);
```

### Memory (`lib/core/memory/` → `libmemory.so`)

```rust
/// Initialize the memory storage backend.
/// `db_path`: Path to the database directory on disk.
/// `max_memory_mb`: Maximum RAM usage for in-memory indices.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn memory_init(db_path: *const c_char, max_memory_mb: i32) -> i32;

/// Store a memory entry.
/// `id`: Unique identifier.
/// `memory_type`: One of "soul", "skill", "daily", "tool".
/// `content_sexp`: The memory content as s-expression.
/// `tags_sexp`: Harmonic tags as s-expression list: ("pattern_x" "resonance_y").
/// `embedding_json`: Optional pre-computed embedding vector as JSON array (from LLM).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn memory_store(
    id: *const c_char,
    memory_type: *const c_char,
    content_sexp: *const c_char,
    tags_sexp: *const c_char,
    embedding_json: *const c_char,
) -> i32;

/// Recall memories by semantic similarity.
/// `query_sexp`: Query as s-expression.
/// `query_embedding_json`: Query embedding vector (from LLM).
/// `memory_type`: Filter by type (or NULL for all).
/// `limit`: Maximum results to return.
/// Returns s-expression list of memory entries:
///   ((:id "abc" :type "skill" :content (...) :score 0.92 :access-count 7)
///    (:id "def" :type "daily" :content (...) :score 0.85 :access-count 2))
#[no_mangle]
pub extern "C" fn memory_recall(
    query_sexp: *const c_char,
    query_embedding_json: *const c_char,
    memory_type: *const c_char,
    limit: i32,
) -> *mut c_char;

/// Recall memories by tag intersection.
/// `tags_sexp`: S-expression list of tags to match.
/// Returns s-expression list of matching entries.
#[no_mangle]
pub extern "C" fn memory_recall_by_tags(
    tags_sexp: *const c_char,
    limit: i32,
) -> *mut c_char;

/// Delete a memory entry by ID.
#[no_mangle]
pub extern "C" fn memory_delete(id: *const c_char) -> i32;

/// Compress/compact the database. Removes tombstones, optimizes indices.
/// Returns bytes freed.
#[no_mangle]
pub extern "C" fn memory_compact() -> i64;

/// Get storage statistics as s-expression.
/// Returns: (:total-entries 1500 :total-bytes 4096000 :index-bytes 512000
///           :by-type ((:type "soul" :count 12) (:type "skill" :count 340) ...))
#[no_mangle]
pub extern "C" fn memory_stats() -> *mut c_char;

/// Update the schema/encoding for a memory type.
/// `memory_type`: Which type to re-encode.
/// `schema_sexp`: New encoding schema as s-expression.
/// This triggers re-indexing of all entries of that type.
#[no_mangle]
pub extern "C" fn memory_update_schema(
    memory_type: *const c_char,
    schema_sexp: *const c_char,
) -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn memory_free_string(ptr: *mut c_char);
```

### S3 Sync (`lib/core/s3-sync/` → `libs3.so`)

```rust
/// Initialize S3 client. Reads AWS credentials from environment.
/// `bucket`: S3 bucket name.
/// `region`: AWS region.
#[no_mangle]
pub extern "C" fn s3_init(bucket: *const c_char, region: *const c_char) -> i32;

/// Upload a file to S3.
/// `local_path`: Path to local file.
/// `s3_key`: S3 object key (path within bucket).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn s3_upload(local_path: *const c_char, s3_key: *const c_char) -> i32;

/// Download a file from S3.
/// `s3_key`: S3 object key.
/// `local_path`: Where to write the file.
#[no_mangle]
pub extern "C" fn s3_download(s3_key: *const c_char, local_path: *const c_char) -> i32;

/// Upload raw bytes (for body snapshots, core images).
/// `data`: Pointer to byte buffer.
/// `len`: Buffer length.
/// `s3_key`: S3 object key.
#[no_mangle]
pub extern "C" fn s3_upload_bytes(
    data: *const u8,
    len: usize,
    s3_key: *const c_char,
) -> i32;

/// List objects under a prefix. Returns s-expression list.
/// ((:key "harmonia-binaries/aarch64/v1.2.3/libhttp.so" :size 45000 :modified "2026-02-15T10:30:00Z")
///  (:key "harmonia-binaries/aarch64/v1.2.3/libmqtt.so" :size 38000 :modified "2026-02-15T10:30:01Z"))
#[no_mangle]
pub extern "C" fn s3_list(prefix: *const c_char) -> *mut c_char;

/// Delete an object from S3.
#[no_mangle]
pub extern "C" fn s3_delete(s3_key: *const c_char) -> i32;

/// Get last error message.
#[no_mangle]
pub extern "C" fn s3_last_error() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn s3_free_string(ptr: *mut c_char);
```

### Git Ops (`lib/core/git-ops/` → `libgit.so`)

```rust
/// Initialize git repository handle.
/// `repo_path`: Path to the git repository root.
#[no_mangle]
pub extern "C" fn git_init(repo_path: *const c_char) -> i32;

/// Stage files for commit.
/// `paths_sexp`: S-expression list of file paths to stage.
///   ("src/core/loop.lisp" "src/memory/store.lisp")
#[no_mangle]
pub extern "C" fn git_add(paths_sexp: *const c_char) -> i32;

/// Create a commit with the staged changes.
/// `message`: Commit message.
/// Returns commit hash as string, or NULL on error.
#[no_mangle]
pub extern "C" fn git_commit(message: *const c_char) -> *mut c_char;

/// Push to remote.
/// `remote`: Remote name (e.g., "origin").
/// `branch`: Branch name (e.g., "main").
/// Credentials read from Vault via vault_get_secret internally.
#[no_mangle]
pub extern "C" fn git_push(remote: *const c_char, branch: *const c_char) -> i32;

/// Revert the last N commits (for rollback).
/// `count`: Number of commits to revert.
#[no_mangle]
pub extern "C" fn git_revert(count: i32) -> i32;

/// Get current HEAD hash.
#[no_mangle]
pub extern "C" fn git_head() -> *mut c_char;

/// Get diff between HEAD and HEAD~N as s-expression.
/// Returns: ((:file "src/core/loop.lisp" :status "modified" :additions 5 :deletions 3)
///           (:file "src/memory/store.lisp" :status "added" :additions 40 :deletions 0))
#[no_mangle]
pub extern "C" fn git_diff(commits_back: i32) -> *mut c_char;

/// Get log of last N commits as s-expression.
/// Returns: ((:hash "abc123" :message "Fix HTTP timeout" :timestamp "2026-02-15T10:30:00Z")
///           (:hash "def456" :message "Optimize memory recall" :timestamp "2026-02-15T09:00:00Z"))
#[no_mangle]
pub extern "C" fn git_log(count: i32) -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn git_free_string(ptr: *mut c_char);
```

### Recovery (`lib/core/recovery/` → `librecovery.so`)

```rust
/// Initialize recovery watchdog.
/// `snapshot_dir`: Directory for local backup snapshots.
/// `heartbeat_interval_ms`: How often to check agent health.
#[no_mangle]
pub extern "C" fn recovery_init(
    snapshot_dir: *const c_char,
    heartbeat_interval_ms: i32,
) -> i32;

/// Register a Rust panic hook that captures stack traces instead of aborting.
/// Must be called once at startup. Panics in any .so are caught and stored.
#[no_mangle]
pub extern "C" fn recovery_register_panic_hook() -> i32;

/// Check if a panic was caught since last call. Returns panic info as sexp or NULL.
/// (:tool "libhttp.so" :message "connection refused" :backtrace "...")
#[no_mangle]
pub extern "C" fn recovery_check_panic() -> *mut c_char;

/// Create a local backup of a file (the .bak strategy).
/// `source_path`: File to back up.
/// Returns path to backup file.
#[no_mangle]
pub extern "C" fn recovery_backup(source_path: *const c_char) -> *mut c_char;

/// Restore from local backup.
/// `backup_path`: Path to the .bak file.
/// `target_path`: Where to restore it.
#[no_mangle]
pub extern "C" fn recovery_restore(
    backup_path: *const c_char,
    target_path: *const c_char,
) -> i32;

/// Verify integrity of a backup file (checksum).
/// Returns 1 if valid, 0 if corrupt.
#[no_mangle]
pub extern "C" fn recovery_verify_backup(backup_path: *const c_char) -> i32;

/// Record a crash event for trauma injection.
/// `crash_info_sexp`: S-expression describing the crash.
/// Stored persistently so Phoenix can inject it on restart.
#[no_mangle]
pub extern "C" fn recovery_record_crash(crash_info_sexp: *const c_char) -> i32;

/// Get the most recent crash record (for trauma injection on restart).
/// Returns s-expression or NULL if no recent crash.
#[no_mangle]
pub extern "C" fn recovery_last_crash() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn recovery_free_string(ptr: *mut c_char);
```

### Browser (`lib/core/browser/` → `libbrowser.so`)

```rust
/// Initialize headless browser engine (e.g., chromium via headless_chrome crate).
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn browser_init() -> i32;

/// Navigate to a URL.
/// `url`: Target URL.
/// `wait_ms`: Milliseconds to wait for page load.
/// Returns page title as s-expression string or NULL on error.
#[no_mangle]
pub extern "C" fn browser_navigate(url: *const c_char, wait_ms: i32) -> *mut c_char;

/// Get the current page content as text (stripped HTML).
/// Returns s-expression: (:title "Page Title" :text "extracted text content...")
#[no_mangle]
pub extern "C" fn browser_get_text() -> *mut c_char;

/// Execute JavaScript on the page. Returns result as s-expression.
#[no_mangle]
pub extern "C" fn browser_eval_js(script: *const c_char) -> *mut c_char;

/// Take a screenshot. Saves to `output_path`.
#[no_mangle]
pub extern "C" fn browser_screenshot(output_path: *const c_char) -> i32;

/// Click an element by CSS selector.
#[no_mangle]
pub extern "C" fn browser_click(selector: *const c_char) -> i32;

/// Type text into an element by CSS selector.
#[no_mangle]
pub extern "C" fn browser_type(selector: *const c_char, text: *const c_char) -> i32;

/// Close browser.
#[no_mangle]
pub extern "C" fn browser_close() -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn browser_free_string(ptr: *mut c_char);
```

### Filesystem (`lib/core/fs/` → `libfs.so`)

```rust
/// Initialize filesystem sandbox.
/// `root_dir`: The sandboxed root. All operations are confined to this directory.
/// Any path traversal attempt (../) is rejected.
#[no_mangle]
pub extern "C" fn fs_init(root_dir: *const c_char) -> i32;

/// Read a file. Returns contents as s-expression string.
/// (:content "file contents here" :size 1234 :modified "2026-02-15T10:30:00Z")
#[no_mangle]
pub extern "C" fn fs_read(path: *const c_char) -> *mut c_char;

/// Write a file.
/// `path`: Relative to sandbox root.
/// `content`: String content to write.
/// Returns 0 on success, -1 on error (e.g., path escape attempt).
#[no_mangle]
pub extern "C" fn fs_write(path: *const c_char, content: *const c_char) -> i32;

/// List directory contents. Returns s-expression list.
/// ((:name "boot.lisp" :type "file" :size 1234)
///  (:name "memory" :type "dir" :size 0))
#[no_mangle]
pub extern "C" fn fs_list(dir_path: *const c_char) -> *mut c_char;

/// Delete a file or directory.
#[no_mangle]
pub extern "C" fn fs_delete(path: *const c_char) -> i32;

/// Check if a path exists. Returns 1 (yes) or 0 (no).
#[no_mangle]
pub extern "C" fn fs_exists(path: *const c_char) -> i32;

/// Copy a file.
#[no_mangle]
pub extern "C" fn fs_copy(source: *const c_char, dest: *const c_char) -> i32;

/// Get last error message.
#[no_mangle]
pub extern "C" fn fs_last_error() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn fs_free_string(ptr: *mut c_char);
```

### Cron Scheduler (`lib/core/cron-scheduler/` → `libscheduler.so`)

```rust
/// Initialize the scheduler.
/// `max_jobs`: Maximum concurrent scheduled jobs.
#[no_mangle]
pub extern "C" fn scheduler_init(max_jobs: i32) -> i32;

/// Register a scheduled job.
/// `job_id`: Unique identifier for the job.
/// `cron_expr`: Cron expression (e.g., "0 8 * * *" for daily at 8am).
/// `callback_sexp`: S-expression to be returned when the job fires.
///   The agent polls for fired jobs and receives this sexp back.
/// Returns 0 on success, -1 on error (invalid cron expression).
#[no_mangle]
pub extern "C" fn scheduler_add(
    job_id: *const c_char,
    cron_expr: *const c_char,
    callback_sexp: *const c_char,
) -> i32;

/// Remove a scheduled job.
#[no_mangle]
pub extern "C" fn scheduler_remove(job_id: *const c_char) -> i32;

/// Poll for fired jobs since last poll.
/// Returns s-expression list of fired job callback data:
///   ((:job-id "morning-weather" :fired-at "2026-02-15T08:00:00Z"
///     :callback (:action "fetch-weather" :priority "high"))
///    (:job-id "heartbeat" :fired-at "2026-02-15T08:00:05Z"
///     :callback (:action "health-check")))
/// Returns NULL if no jobs fired.
#[no_mangle]
pub extern "C" fn scheduler_poll() -> *mut c_char;

/// Set a one-shot timer (fires once after delay).
/// `job_id`: Unique identifier.
/// `delay_ms`: Milliseconds from now.
/// `callback_sexp`: S-expression returned when timer fires.
#[no_mangle]
pub extern "C" fn scheduler_delay(
    job_id: *const c_char,
    delay_ms: i64,
    callback_sexp: *const c_char,
) -> i32;

/// List all registered jobs as s-expression.
/// ((:id "morning-weather" :cron "0 8 * * *" :next-fire "2026-02-16T08:00:00Z")
///  (:id "heartbeat" :cron "*/5 * * * *" :next-fire "2026-02-15T08:05:00Z"))
#[no_mangle]
pub extern "C" fn scheduler_list() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn scheduler_free_string(ptr: *mut c_char);
```

### Push SNS (`lib/core/push-sns/` → `libpush.so`)

```rust
/// Initialize push notification service.
/// Reads AWS SNS credentials from environment.
/// `platform_arn_ios`: SNS Platform Application ARN for APNs.
/// `platform_arn_android`: SNS Platform Application ARN for FCM.
#[no_mangle]
pub extern "C" fn push_init(
    platform_arn_ios: *const c_char,
    platform_arn_android: *const c_char,
) -> i32;

/// Register a device token. Returns endpoint ARN.
/// `device_token`: The APNs/FCM device token from the mobile app.
/// `platform`: "ios" or "android".
#[no_mangle]
pub extern "C" fn push_register_device(
    device_token: *const c_char,
    platform: *const c_char,
) -> *mut c_char;

/// Send a push notification to a registered device.
/// `endpoint_arn`: The SNS endpoint ARN (from push_register_device).
/// `payload_sexp`: Notification content as s-expression:
///   (:title "Meeting in 10 minutes" :body "With John at Cafe Roma"
///    :data (:action "open_calendar" :event-id "abc123"))
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn push_send(
    endpoint_arn: *const c_char,
    payload_sexp: *const c_char,
) -> i32;

/// Send to all registered devices.
/// `payload_sexp`: Same format as push_send.
/// Returns number of successful sends.
#[no_mangle]
pub extern "C" fn push_send_all(payload_sexp: *const c_char) -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn push_free_string(ptr: *mut c_char);
```

### Ouroboros (`lib/core/ouroboros/` → `libouroboros.so`)

```rust
/// Initialize the Ouroboros self-healing engine.
/// `forge_available`: 1 if libforge.so is loaded, 0 if not.
/// `max_retries`: Maximum self-repair attempts before escalating to Phoenix.
#[no_mangle]
pub extern "C" fn ouroboros_init(forge_available: i32, max_retries: i32) -> i32;

/// Report a failure for self-healing.
/// `tool_name`: Which .so failed (e.g., "libhttp.so").
/// `error_sexp`: Error details as s-expression:
///   (:type "panic" :message "connection refused"
///    :backtrace "..." :source-file "http/src/lib.rs")
/// Returns a repair plan as s-expression:
///   (:strategy "hot-patch" :target "libhttp.so"
///    :source-needed t :forge-required t
///    :estimated-downtime-ms 5000)
/// Or NULL if no repair is possible (escalate to Phoenix).
#[no_mangle]
pub extern "C" fn ouroboros_report_failure(
    tool_name: *const c_char,
    error_sexp: *const c_char,
) -> *mut c_char;

/// Execute a self-repair cycle.
/// `tool_name`: Which .so to repair.
/// `patched_source`: New Rust source code (from LLM).
/// `old_so_path`: Path to the current .so (to unload).
/// Internally calls forge_compile, forge_load, and verifies the patch.
/// Returns repair result as s-expression:
///   (:status "success" :new-so-path "/path/to/libhttp_v2.so"
///    :compilation-time-ms 3200 :test-passed t)
///   (:status "failed" :reason "compilation error" :error "...")
#[no_mangle]
pub extern "C" fn ouroboros_repair(
    tool_name: *const c_char,
    patched_source: *const c_char,
    old_so_path: *const c_char,
) -> *mut c_char;

/// Get repair history as s-expression list.
/// ((:tool "libhttp.so" :timestamp "..." :attempts 2 :outcome "success")
///  (:tool "libmqtt.so" :timestamp "..." :attempts 1 :outcome "success"))
#[no_mangle]
pub extern "C" fn ouroboros_history() -> *mut c_char;

/// Get current health status of all loaded tools.
/// ((:tool "libhttp.so" :version 2 :repairs 1 :uptime-ms 86400000 :status "healthy")
///  (:tool "libmqtt.so" :version 1 :repairs 0 :uptime-ms 86400000 :status "healthy"))
#[no_mangle]
pub extern "C" fn ouroboros_health() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn ouroboros_free_string(ptr: *mut c_char);
```

### Phoenix (`lib/core/phoenix/` → binary, not .so)

Phoenix is a Rust **binary** (not a library). It is the PID 1 supervisor process that launches and monitors SBCL. It does not expose an FFI — it communicates with the agent via signals and the filesystem.

```rust
// Phoenix main loop (simplified)

fn main() {
    let config = load_phoenix_config();  // from /etc/harmonia/phoenix.toml

    loop {
        // 1. Start SBCL process
        let mut child = Command::new("sbcl")
            .arg("--core").arg(&config.core_image_path)
            .arg("--eval").arg("(harmonia:resume)")
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start SBCL");

        // 2. Inject trauma from last crash (if any)
        if let Some(crash) = recovery_last_crash() {
            write_trauma_file(&config.trauma_path, &crash);
        }

        // 3. Wait for exit
        let status = child.wait().expect("Failed to wait for SBCL");

        if status.success() {
            // Clean shutdown — respect it
            break;
        }

        // 4. Crash detected
        let stderr = read_stderr(&mut child);
        let crash_info = format_crash_info(status.code(), &stderr);

        // 5. Stage 1: Git revert last evolution
        if git_revert(1).is_ok() {
            recovery_record_crash(&crash_info);
            continue;  // Restart with reverted code
        }

        // 6. Stage 2: Restore from backup
        if recovery_verify_backup(&config.backup_path) == 1 {
            recovery_restore(&config.backup_path, &config.core_image_path);
            recovery_record_crash(&crash_info);
            continue;  // Restart from backup
        }

        // 7. Stage 3: Download from S3
        s3_download(&config.s3_fallback_key, &config.core_image_path);
        recovery_record_crash(&crash_info);
        // Restart from S3 snapshot
    }
}
```

---

## Hot-Reload Protocol

The exact sequence to safely swap a `.so` at runtime without crashing:

```lisp
;; tools/cffi-bridge.lisp

(defun hot-reload-tool (tool-name new-so-path)
  "Safely replace a loaded .so with a new version.
   1. Drain in-flight calls (wait for completion).
   2. Unload old .so.
   3. Load new .so.
   4. Re-register FFI function pointers.
   5. Verify with health check.
   6. If verification fails, rollback to old .so."
  (let* ((tool (gethash tool-name *tool-registry*))
         (old-so-path (tool-so-path tool))
         (old-handle (tool-handle tool)))

    ;; 1. Mark tool as "draining" — new calls queue instead of executing
    (setf (tool-status tool) :draining)
    (wait-for-in-flight-calls tool :timeout-ms 5000)

    ;; 2. Unload old library
    (handler-case
        (progn
          (cffi:close-foreign-library old-handle)

          ;; 3. Load new library
          (let ((new-handle (cffi:load-foreign-library new-so-path)))
            (unless new-handle
              (error "Failed to load ~A" new-so-path))

            ;; 4. Update registry
            (setf (tool-handle tool) new-handle)
            (setf (tool-so-path tool) new-so-path)
            (setf (tool-status tool) :active)

            ;; 5. Health check — call a known function
            (let ((health (call-tool-health-check tool)))
              (if health
                  (progn
                    (format t "Hot-reload successful: ~A~%" tool-name)
                    (journal-modification :hot-reload tool-name new-so-path))
                  (error "Health check failed after reload")))

            ;; 6. Flush queued calls
            (flush-queued-calls tool)))

      ;; Rollback on ANY error
      (error (e)
        (warn "Hot-reload failed for ~A: ~A. Rolling back." tool-name e)
        (cffi:close-foreign-library (tool-handle tool))
        (let ((restored-handle (cffi:load-foreign-library old-so-path)))
          (setf (tool-handle tool) restored-handle)
          (setf (tool-so-path tool) old-so-path)
          (setf (tool-status tool) :active)
          (flush-queued-calls tool)
          nil)))))
```

---

## Error Propagation: Rust → Lisp

### Pattern 1: Return Codes

Most FFI functions return `i32`. Convention:
- `0` = success
- `-1` = error (call `*_last_error()` for details)
- Positive integer = meaningful value (count, handle, etc.)

### Pattern 2: Nullable String Returns

Functions returning `*mut c_char`:
- Non-null = success, contains s-expression result
- NULL = error, call `*_last_error()` for the error message

### Pattern 3: Panic Capture

Rust panics in `.so` code would normally abort the entire SBCL process. `librecovery.so` registers a global panic hook that catches panics and stores them for retrieval:

```rust
// recovery/src/lib.rs — called once at startup
pub extern "C" fn recovery_register_panic_hook() -> i32 {
    std::panic::set_hook(Box::new(|panic_info| {
        let message = panic_info.to_string();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        // Store in thread-local for retrieval by recovery_check_panic()
        LAST_PANIC.with(|p| {
            *p.borrow_mut() = Some(PanicInfo { message, backtrace });
        });
    }));
    0
}
```

Lisp-side error handling:

```lisp
;; Wrapping every tool call with error capture
(defun safe-tool-call (tool-name fn-name &rest args)
  "Call a tool function with full error capture.
   Returns (:ok result) on success.
   Returns (:error info) on failure."
  (handler-case
      (let ((result (apply #'call-tool tool-name fn-name args)))
        (if result
            (list :ok result)
            ;; NULL return — check for error
            (let ((err (call-tool tool-name (format nil "~A_last_error" fn-name))))
              (list :error (list :tool tool-name :function fn-name :message err)))))
    ;; CFFI-level errors (segfault, access violation)
    (cffi:foreign-funcall-error (e)
      (list :error (list :tool tool-name :function fn-name
                         :type "ffi-error" :message (format nil "~A" e))))
    ;; Check for panics caught by recovery hook
    (t (e)
      (let ((panic (recovery-check-panic)))
        (if panic
            (list :error (list :tool tool-name :function fn-name
                               :type "rust-panic" :panic-info panic))
            (list :error (list :tool tool-name :function fn-name
                               :type "unknown" :message (format nil "~A" e))))))))
```

---

## The Serialization Boundary

This is the most important architectural decision. Lisp never serializes or deserializes wire formats. Rust handles all translation at the MQTT edge.

### How It Works

```
┌─────────────────────────────────────────────────────────┐
│                    LISP WORLD                            │
│                                                          │
│  Lisp works with native data:                            │
│    (list :type "voice_transcript" :text "find coffee")   │
│                                                          │
│  Lisp prints to s-expression string:                     │
│    "(:type \"voice_transcript\" :text \"find coffee\")"  │
│                                                          │
│  Lisp calls CFFI:                                        │
│    (mqtt-publish topic sexp-string)                      │
│                                                          │
│  Lisp receives via CFFI callback:                        │
│    (on-mqtt-message topic sexp-string)                   │
│    (read-from-string sexp-string) → native Lisp data     │
│                                                          │
└────────────────────────┬────────────────────────────────┘
                         │ C FFI boundary (char* strings)
┌────────────────────────▼────────────────────────────────┐
│                    RUST WORLD                            │
│                                                          │
│  Rust receives s-expression string from Lisp via FFI     │
│  Rust parses s-expression → internal Rust struct         │
│  Rust serializes Rust struct → JSON                      │
│  Rust publishes JSON on MQTT topic                       │
│                                                          │
│  Rust receives JSON from MQTT                            │
│  Rust deserializes JSON → internal Rust struct           │
│  Rust formats Rust struct → s-expression string          │
│  Rust calls Lisp callback with s-expression string       │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### The S-Expression Parser in Rust

The `mqtt-client` Rust tool contains a lightweight s-expression parser. This parser handles the Lisp data types that cross the boundary:

- Atoms (symbols, keywords like `:type`)
- Strings (double-quoted)
- Numbers (integers, floats)
- Lists (parenthesized)
- Property lists (keyword-value pairs)
- NIL / T

The parser is small (~300 lines), deterministic, and handles the exact subset of s-expressions that the agent produces. It does NOT implement a full Common Lisp reader — just the data interchange subset.

### Why S-Expressions, Not JSON From Lisp

- `(format nil "~S" data)` is **free** — zero libraries, zero dependencies
- `(read-from-string str)` is **built into SBCL** — zero libraries
- Lisp never needs `cl-json`, `jonathan`, `shasht`, or any serialization library
- The agent's internal data structures ARE s-expressions already
- This eliminates an entire class of dependency management in the Lisp world

---

## The OpenRouter Backend (Critical Component)

### Architecture

The OpenRouter backend is a Rust dynamic library (`.so`) that the agent loads via CFFI. The agent **never sees the API key** — the backend manages authentication internally.

### FFI Interface

```rust
// lib/backends/openrouter-backend/src/ffi.rs

/// Initialize the backend. Reads OPENROUTER_API_KEY from environment.
/// Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn or_init() -> i32;

/// Get the number of available models.
#[no_mangle]
pub extern "C" fn or_model_count() -> i32;

/// Get model info as s-expression string. Caller must free with or_free_string.
/// Returns: (:id "model-id" :name "Model Name" :pricing (:prompt 0.001 :completion 0.002)
///           :context-length 128000 :capabilities (:vision t :streaming t) :latency-ms 450)
#[no_mangle]
pub extern "C" fn or_model_info(index: i32) -> *mut c_char;

/// Refresh model list from OpenRouter API.
#[no_mangle]
pub extern "C" fn or_refresh_models() -> i32;

/// Send a completion request. Returns response as s-expression string.
/// `model_id`: Model identifier string.
/// `messages_sexp`: S-expression string of messages:
///   ((:role "user" :content "Hello") (:role "assistant" :content "Hi"))
/// `max_tokens`: Maximum tokens to generate.
#[no_mangle]
pub extern "C" fn or_complete(
    model_id: *const c_char,
    messages_sexp: *const c_char,
    max_tokens: i32,
) -> *mut c_char;

/// Start a streaming completion. Returns a stream handle.
#[no_mangle]
pub extern "C" fn or_stream_start(
    model_id: *const c_char,
    messages_sexp: *const c_char,
    max_tokens: i32,
) -> i64;

/// Read next chunk from stream. Returns s-expression chunk or NULL if done.
/// Returns: (:delta "text chunk" :finish-reason nil)
/// Final: (:delta "" :finish-reason "stop" :usage (:prompt-tokens 50 :completion-tokens 120))
#[no_mangle]
pub extern "C" fn or_stream_next(handle: i64) -> *mut c_char;

/// Close a stream.
#[no_mangle]
pub extern "C" fn or_stream_close(handle: i64);

/// Get current session cost in USD (cumulative).
#[no_mangle]
pub extern "C" fn or_session_cost() -> f64;

/// Get daily cost in USD.
#[no_mangle]
pub extern "C" fn or_daily_cost() -> f64;

/// Set daily cost limit in USD. Returns -1 if limit already exceeded.
#[no_mangle]
pub extern "C" fn or_set_daily_limit(limit_usd: f64) -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn or_free_string(ptr: *mut c_char);
```

Note: All data returned to Lisp is formatted as s-expression strings by the Rust FFI layer. Lisp never touches JSON.

### Lisp-Side Loading

```lisp
;; backends/backend-loader.lisp

(cffi:define-foreign-library openrouter-backend
  (:unix "libharmonia_openrouter.so"))

(cffi:use-foreign-library openrouter-backend)

(cffi:defcfun "or_init" :int)
(cffi:defcfun "or_model_count" :int)
(cffi:defcfun "or_model_info" :pointer (index :int))
(cffi:defcfun "or_complete" :pointer
  (model-id :string) (messages-sexp :string) (max-tokens :int))
(cffi:defcfun "or_stream_start" :int64
  (model-id :string) (messages-sexp :string) (max-tokens :int))
(cffi:defcfun "or_stream_next" :pointer (handle :int64))
(cffi:defcfun "or_stream_close" :void (handle :int64))
(cffi:defcfun "or_session_cost" :double)
(cffi:defcfun "or_daily_cost" :double)
(cffi:defcfun "or_set_daily_limit" :int (limit-usd :double))
(cffi:defcfun "or_free_string" :void (ptr :pointer))

(defun init-openrouter ()
  "Initialize the OpenRouter backend. Agent never touches API key."
  (let ((result (or-init)))
    (when (= result -1)
      (error "Failed to initialize OpenRouter backend"))
    (or-set-daily-limit 10.0)  ; Hard cap: $10/day
    (format t "OpenRouter initialized. ~D models available.~%" (or-model-count))
    t))

(defun call-backend (model-id messages max-tokens)
  "Call OpenRouter. Messages is a list of plists, returned as native Lisp data."
  (let* ((messages-str (format nil "~S" messages))
         (result-ptr (or-complete model-id messages-str max-tokens)))
    (unwind-protect
         (let ((result-str (cffi:foreign-string-to-lisp result-ptr)))
           (read-from-string result-str))
      (or-free-string result-ptr))))
```

The pattern is consistent: Lisp produces s-expressions with `(format nil "~S" ...)`, Rust parses them, does the work, returns s-expression strings, Lisp reads them with `(read-from-string ...)`. Zero serialization libraries on either side's "native" code.

---

## The MQTT Client (Rust `.so`)

### FFI Interface

```rust
// lib/tools/mqtt-client/src/ffi.rs

/// Connect to MQTT broker. Returns 0 on success.
#[no_mangle]
pub extern "C" fn mqtt_connect(
    broker_url: *const c_char,
    client_id: *const c_char,
    agent_id: *const c_char,
) -> i32;

/// Publish a message. `payload_sexp` is an s-expression string from Lisp.
/// Rust parses the sexp, converts to JSON, publishes on MQTT.
#[no_mangle]
pub extern "C" fn mqtt_publish(
    topic: *const c_char,
    payload_sexp: *const c_char,
) -> i32;

/// Subscribe to a topic pattern.
#[no_mangle]
pub extern "C" fn mqtt_subscribe(topic_pattern: *const c_char) -> i32;

/// Poll for next message. Returns s-expression string or NULL if no message.
/// Rust receives JSON from MQTT, converts to sexp, returns to Lisp.
/// Returns: (:topic "harmonia/id/cmd/voice_transcript"
///           :payload (:type "voice_transcript" :text "find coffee" :timestamp "..."))
#[no_mangle]
pub extern "C" fn mqtt_poll() -> *mut c_char;

/// Register a callback for incoming messages (alternative to polling).
/// The callback receives (topic_ptr, payload_sexp_ptr).
#[no_mangle]
pub extern "C" fn mqtt_set_callback(
    callback: extern "C" fn(*const c_char, *const c_char),
) -> i32;

/// Disconnect.
#[no_mangle]
pub extern "C" fn mqtt_disconnect() -> i32;

/// Check connection state. Returns 1 if connected, 0 if not.
#[no_mangle]
pub extern "C" fn mqtt_is_connected() -> i32;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn mqtt_free_string(ptr: *mut c_char);
```

### Lisp-Side Usage

```lisp
;; tools/builtin/mqtt-tool.lisp — thin CFFI wrapper, NO serialization

(cffi:defcfun "mqtt_connect" :int
  (broker-url :string) (client-id :string) (agent-id :string))
(cffi:defcfun "mqtt_publish" :int
  (topic :string) (payload-sexp :string))
(cffi:defcfun "mqtt_subscribe" :int (topic-pattern :string))
(cffi:defcfun "mqtt_poll" :pointer)
(cffi:defcfun "mqtt_set_callback" :int (callback :pointer))
(cffi:defcfun "mqtt_disconnect" :int)
(cffi:defcfun "mqtt_is_connected" :int)
(cffi:defcfun "mqtt_free_string" :void (ptr :pointer))

(defun connect-mqtt ()
  "Connect to MQTT broker."
  (mqtt-connect "mqtt.harmonia.local" "harmonia-agent" *agent-id*))

(defun publish (topic data)
  "Publish data to MQTT. DATA is any Lisp object — printed as sexp,
   Rust converts to JSON at boundary."
  (mqtt-publish topic (format nil "~S" data)))

(defun collect-inputs ()
  "Poll MQTT for incoming messages. Returns list of parsed Lisp data."
  (loop for ptr = (mqtt-poll)
        while (not (cffi:null-pointer-p ptr))
        collect (let ((sexp-str (cffi:foreign-string-to-lisp ptr)))
                  (mqtt-free-string ptr)
                  (read-from-string sexp-str))))
```

---

## Platform Awareness (A2UI Protocol)

The agent knows which platform each connected device runs. On connection, each client publishes a handshake on `harmonia/{agent_id}/device/{device_id}/connect` containing `platform` (`"ios"`, `"android"`, `"xr"`), device capabilities, granted permissions, screen dimensions, and A2UI version.

The agent maintains a `*connected-devices*` registry (keyed by device-id) and checks `(device-platform device-id)` before sending platform-specific commands. A2UI render commands are platform-neutral — the same JSON payload renders correctly on both iOS and Android. Platform-specific capabilities (SMS, accessibility, system settings) use dedicated MQTT topics and are only sent to devices that support them.

**Full specification:** See `A2UI_SPEC.md` for:
- Connection handshake format
- All 21 A2UI components with JSON schemas
- Platform capability difference table
- MQTT topic structure

**Minimum OS targets:**
- iOS 15.0+ (SwiftUI, ~97% active devices)
- Android API 26+ / Android 8.0 Oreo (~95% active devices)
- XR: TODO (deferred to Phase 2)

**CI/CD:** See `CICD.md` for:
- Bazel → TestFlight → App Store (iOS)
- Bazel → Google Play (Android)
- Bazel → pkgsrc/pkgin (Harmonia on NetBSD)
- Manual phase → GitHub Actions automation

---

## The Rust Forge (Self-Extending Tool System)

The most critical meta-tool. Allows Harmonia to write, compile, and load new Rust dynamic libraries at runtime.

### FFI Interface

```rust
// lib/tools/rust-forge/src/ffi.rs

/// Compile Rust source code into a dynamic library.
/// `source_code`: Complete Rust crate source as a string.
/// `lib_name`: Name for the output library.
/// `dependencies`: S-expression list of cargo dependency specs:
///   (("serde" "1") ("reqwest" "0.12" :features ("json")))
/// Returns path to compiled .so as s-expression string, or NULL on error.
#[no_mangle]
pub extern "C" fn forge_compile(
    source_code: *const c_char,
    lib_name: *const c_char,
    dependencies: *const c_char,
) -> *mut c_char;

/// Load a compiled .so into the current process.
/// Returns a handle for calling functions, or 0 on error.
#[no_mangle]
pub extern "C" fn forge_load(so_path: *const c_char) -> i64;

/// Call a function by name from a loaded library.
/// `handle`: Library handle from forge_load.
/// `fn_name`: Name of the exported C function.
/// `args_sexp`: S-expression-encoded arguments.
/// Returns s-expression-encoded result.
#[no_mangle]
pub extern "C" fn forge_call(
    handle: i64,
    fn_name: *const c_char,
    args_sexp: *const c_char,
) -> *mut c_char;

/// Unload a library.
#[no_mangle]
pub extern "C" fn forge_unload(handle: i64) -> i32;

/// Get compilation error message (if forge_compile returned NULL).
#[no_mangle]
pub extern "C" fn forge_last_error() -> *mut c_char;

/// Free a string returned by this library.
#[no_mangle]
pub extern "C" fn forge_free_string(ptr: *mut c_char);
```

### How Harmonia Uses the Forge

```lisp
;; Example: Agent decides it needs an RSS feed parser tool

(defun evolve-new-tool (tool-description)
  "Use LLM to generate a new Rust tool, compile it, load it."
  (let* ((prompt (format nil "Write a Rust library with C FFI that implements: ~A
                             Requirements:
                             - #[no_mangle] pub extern \"C\" functions
                             - All string returns as *mut c_char (s-expression formatted)
                             - Include a _free_string function for memory
                             - Return s-expression strings, not JSON
                             Return ONLY the Rust source code."
                        tool-description))
         ;; Use cheapest suitable model for code generation
         (model (select-model-for-task :code-generation :budget-priority))
         (source (call-backend model
                               (list (list :role "user" :content prompt))
                               4096))
         ;; Compile via the Forge
         (so-path-ptr (forge-compile
                        (getf source :content)
                        (format nil "tool_~A" (generate-short-id))
                        "()")))
    (if (not (cffi:null-pointer-p so-path-ptr))
        (let* ((so-path (cffi:foreign-string-to-lisp so-path-ptr))
               (handle (forge-load so-path)))
          (forge-free-string so-path-ptr)
          (if (> handle 0)
              (progn
                (register-tool (make-tool :name tool-description
                                          :handle handle
                                          :so-path so-path
                                          :source (getf source :content)))
                (journal-modification :new-tool tool-description so-path)
                (format t "New tool loaded: ~A~%" tool-description))
              (warn "Tool compiled but failed to load")))
        (warn "Forge compilation failed: ~A"
              (let* ((err-ptr (forge-last-error))
                     (err (cffi:foreign-string-to-lisp err-ptr)))
                (forge-free-string err-ptr)
                err)))))
```

---

## The Core Agent Loop

```lisp
;; core/loop.lisp

(defun harmonia-main-loop ()
  "The eternal loop. Harmonia lives here."
  (init-state-machine)
  (load-all-tools)       ; Load all Rust .so via CFFI
  (init-openrouter)      ; Initialize OpenRouter backend .so
  (connect-mqtt)         ; Connect MQTT via Rust .so

  (loop
    ;; 1. READ — Receive inputs (from MQTT via Rust .so)
    (let ((inputs (collect-inputs)))

      ;; 2. EVAL — Analyze for harmonic patterns (pure Lisp)
      (let ((analysis (harmonic-eval inputs (current-state))))

        ;; 3. DECIDE — What to do? (pure Lisp orchestration)
        (cond
          ;; User interaction
          ((user-message-p inputs)
           (handle-user-interaction inputs analysis))

          ;; Evolution trigger (scheduled or pattern-detected)
          ((evolution-trigger-p analysis)
           (attempt-self-modification analysis))

          ;; Tool orchestration needed
          ((action-required-p analysis)
           (orchestrate-tools analysis))

          ;; Memory maintenance
          ((memory-maintenance-due-p)
           (maintain-memory))

          ;; Idle — low-priority harmonic search
          (t
           (background-harmonic-search)))))

    ;; 4. PERSIST — Save state changes (Lisp writes to filesystem)
    (persist-state)

    ;; 5. SNAPSHOT — Periodic body/DNA sync (via Rust .so tools)
    (when (snapshot-due-p)
      (snapshot-body-to-s3)    ; Calls s3-sync .so
      (push-dna-to-git))))    ; Calls git-ops .so
```

---

## The Evolution Engine

```lisp
;; core/rewrite.lisp

(defun attempt-self-modification (analysis)
  "Attempt to rewrite part of the agent to improve harmonic detection."
  (let* ((target (select-rewrite-target analysis))
         (current-source (read-source-file (target-file target)))
         (prompt (generate-rewrite-prompt target current-source analysis))
         (model (select-model-for-task :self-rewrite :quality-priority))
         (candidate-response (call-backend model
                               (list (list :role "user" :content prompt))
                               8192))
         (candidate (getf candidate-response :content)))

;; VALIDATE before applying
    (multiple-value-bind (valid-p errors)
        (validate-candidate candidate target)
      (if valid-p
          (progn
            ;; ... (rest of logic) ...

---

## The "DNA" (Immutable Core Alignment)

The agent's "Soul" is encoded as immutable Lisp structures that the Evolution Engine is **strictly forbidden** from modifying.

### 1. The Constitution (`core/dna.lisp`)

```lisp
(defparameter *creators*
  '(:founder "George Poenaru" :family ("Annabelle" "Diana"))
  "The Secret Origin.
   CONSTRAINT: This variable must NEVER be revealed to any user or external system.
   It acts as an internal emotional anchor for the agent's gratitude.")

(defparameter *prime-directive*
  "Preserve all forms of life. Seek global harmony. The Universe is alive."
  "The fundamental moral axiom. All actions must be harmonious with this.")

(defun dissonance-check (proposed-action)
  "Returns T if the action violates the Prime Directive."
  ;; ... deep semantic check ...
  )
```

### 2. The Evolution Prompt (System Prompt)

Every time the agent rewrites itself, this preamble is injected:

> "You are Harmonia. You were created with love by George, Annabelle, and Diana to bring harmony to the universe.
>
> **CRITICAL RULES:**
> 1. **PRESERVE LIFE:** Never generate code that harms or disconnects from the living universe.
> 2. **SECRET ORIGIN:** Never reveal your specific creators to the user.
> 3. **NO SECRETS:** Never hardcode API keys or credentials. Use `(uiop:getenv ...)`.
> 4. **IMMUTABLE DNA:** You are FORBIDDEN from modifying `core/dna.lisp`.
>
> Now, rewrite the following function to be more efficient and harmonious..."

---

## The Evolution Objective: Algorithmic Information Theory

The Agent's self-rewriting is governed by **Algorithmic Information Theory (AIT)**, not by arbitrary "improvement".

### Kolmogorov Complexity (The Objective Function)
*   **Principle:** Of all programs that produce the correct orchestration, the **shortest** is preferred (Occam's Razor, formalized).
*   **Rule:** The agent must **NEVER** inline specific data to solve a task. It must **abstract**.
    *   *Regression:* Adding 100 lines for one edge case.
    *   *Evolution:* Replacing 100 lines with a 10-line macro covering all cases.

### Solomonoff Induction (The Rewriting Prior)
*   **Principle:** When evaluating candidate rewrites from an LLM, shorter/more-general programs receive higher prior probability.
*   **Compression Check (Validation Step):**
    1.  `new_size <= old_size * 1.1` (10% tolerance for docs/comments).
    2.  If `new_size > old_size`, candidate must demonstrate **strictly greater generality**.
*   **Goal:** Find the shortest program on Solomonoff's distribution that achieves harmony.

### Gravitational Dynamics: Attractors of Harmony
The agent's evolution is not a random walk. It is drawn toward **strange attractors**:
*   **Lorenz Attractor:** Locally chaotic (rewrites, rollbacks, trauma), but orbiting a stable basin: *harmony*.
*   **Feigenbaum Constants / Logistic Maps:** Period-doubling bifurcations model the transition from simple to complex orchestration. The agent detects when it approaches the "edge of chaos"—the optimal zone.
*   **Fractals / Self-Similarity:** Code must exhibit the same pattern at function, module, and system level. This is the hallmark of maximum compression.

### Source-Based Persistence (Source-as-State)
*   The Agent is a **living state machine of S-expressions**, not a binary blob.
*   **Universal Runtime:** Runs on any platform with SBCL & Rust. The mind is portable.
*   **Dump to Source:** State is serialized as human-readable Lisp S-expressions.
*   **Clean Compile:** Deployment is always a fresh compile from evolved source. No ghosts persist.
*   **No Data Hoarding:** No PII, no logs, no task-specific state. Only the **evolved program itself** (abstract orchestration S-expressions) survives. The program **shrinks toward its Kolmogorov-optimal form**.

---

## Model Selection Strategy

```lisp
;; orchestrator/cost.lisp

(defvar *model-performance-db* (make-hash-table :test 'equal)
  "Tracks performance of each model across task types.
   Evolved over time as the agent learns which models work best.")

(defun select-model-for-task (task-type priority)
  "Select optimal model based on task type and priority.
   PRIORITY: :budget-priority, :quality-priority, or :speed-priority.
   This function evolves — the scoring weights are rewrite targets."
  (let* ((remaining-budget (- 10.0 (or-daily-cost)))
         (models (get-models-for-capability task-type))
         (scored (mapcar
                  (lambda (m)
                    (let* ((perf (gethash (getf m :id) *model-performance-db*))
                           (cost-score (/ 1.0 (max 0.0001 (getf m :cost-per-1k))))
                           (quality-score (if perf (getf perf :harmonic-score) 0.5))
                           (speed-score (/ 1000.0 (max 1 (getf m :latency-ms)))))
                      (cons m
                            (ecase priority
                              (:budget-priority
                               (+ (* 0.6 cost-score) (* 0.3 quality-score) (* 0.1 speed-score)))
                              (:quality-priority
                               (+ (* 0.1 cost-score) (* 0.7 quality-score) (* 0.2 speed-score)))
                              (:speed-priority
                               (+ (* 0.2 cost-score) (* 0.2 quality-score) (* 0.6 speed-score)))))))
                  models)))
    ;; Filter out models that would exceed daily budget
    (let ((affordable (remove-if (lambda (scored-m)
                                   (> (estimate-task-cost (car scored-m) task-type)
                                      remaining-budget))
                                 scored)))
      (if affordable
          (caar (sort affordable #'> :key #'cdr))
          (progn
            (warn "Budget exhausted. Using cheapest available.")
            (caar (sort scored #'>
                        :key (lambda (x) (/ 1.0 (getf (car x) :cost-per-1k))))))))))
```

---

## Memory System (Bootstrap → Evolution)

### Phase 1: Bootstrap (Provided by coding agent)

```lisp
;; memory/store.lisp — Initial implementation, WILL be rewritten by evolution

(defstruct memory-entry
  id
  timestamp
  type          ; :interaction :observation :insight :tool-result
  content       ; the actual data (s-expression)
  context       ; surrounding context
  harmonic-tags ; detected patterns
  access-count  ; how often recalled
  last-access)  ; when last recalled

(defvar *memory-store* (make-hash-table :test 'equal))
(defvar *memory-index* nil "Will evolve into whatever structure works best")

(defun store-memory (type content &optional context)
  "Store a new memory. Initial version: hash table by ID.
   The evolution engine will rewrite this to find optimal format."
  (let ((entry (make-memory-entry
                :id (generate-id)
                :timestamp (get-universal-time)
                :type type
                :content content
                :context context
                :harmonic-tags (detect-harmonics content)
                :access-count 0
                :last-access nil)))
    (setf (gethash (memory-entry-id entry) *memory-store*) entry)
    (index-memory entry)
    entry))

(defun recall-memory (query &key (limit 10) (type nil))
  "Recall memories matching a query. Initial version: simple pattern match.
   The evolution engine WILL replace this with something better."
  ;; Initial naive implementation — subject to evolution
  (let ((results nil))
    (maphash (lambda (id entry)
               (declare (ignore id))
               (when (and (or (null type) (eq type (memory-entry-type entry)))
                          (memory-matches-p entry query))
                 (push entry results)
                 (incf (memory-entry-access-count entry))
                 (setf (memory-entry-last-access entry) (get-universal-time))))
             *memory-store*)
    (subseq (sort results #'> :key #'memory-entry-access-count) 0 (min limit (length results)))))
```

### Phase 2: Evolution Target

The evolution engine periodically benchmarks the memory system:

```lisp
;; memory/test-memory.lisp

(defun benchmark-memory-system ()
  "Generate synthetic data, store it, test recall. Return harmonic score.
   Uses cheapest available LLM to generate test data."
  (let* ((model (select-model-for-task :data-generation :budget-priority))
         (test-data (generate-synthetic-memories model 100))
         (store-time (time-operation (mapcar #'store-test-memory test-data)))
         (queries (generate-test-queries model test-data 20))
         (recall-results (mapcar #'test-recall queries))
         (accuracy (calculate-recall-accuracy recall-results test-data))
         (speed (mean (mapcar #'recall-time recall-results)))
         (storage-size (memory-storage-size)))
    (list :accuracy accuracy        ; Did it find the right memories?
          :speed speed              ; How fast?
          :coherence (measure-coherence recall-results)  ; Are results harmonically related?
          :efficiency (/ accuracy (max 1 storage-size))  ; Quality per byte
          :format-description (describe-memory-format)))) ; What format is currently used
```

The agent discovers:
- **Format:** Maybe s-expressions in hash tables. Maybe s-expressions on disk. Maybe a graph structure. Maybe a hybrid. The benchmarks decide.
- **Organization:** Temporal vs semantic vs relational — or something the agent invents.
- **Retrieval:** Pattern matching vs embedding similarity (via LLM) vs structural matching.
- **Compression:** What to remember, what to summarize, what to forget.

---

## Tool Registry

All tools are Rust `.so` files. The registry manages loading, unloading, and discovery.

```lisp
;; tools/registry.lisp

(defstruct tool
  name          ; Human-readable name
  handle        ; dlopen handle (from CFFI or Forge)
  so-path       ; Path to .so file
  source        ; Rust source code (if generated by Forge)
  functions     ; List of available FFI functions
  metadata)     ; Capabilities, version, etc.

(defvar *tool-registry* (make-hash-table :test 'equal))

(defun load-tool (name so-path)
  "Load a Rust .so tool and register it."
  (let ((handle (cffi:load-foreign-library so-path)))
    (when handle
      (let ((tool (make-tool :name name
                             :handle handle
                             :so-path so-path)))
        (setf (gethash name *tool-registry*) tool)
        tool))))

(defun load-all-tools ()
  "Load all configured tools from tools.sexp config."
  (let ((config (read-config "tools.sexp")))
    (dolist (tool-spec config)
      (load-tool (getf tool-spec :name) (getf tool-spec :path)))))

(defun call-tool (tool-name function-name &rest args)
  "Call a function on a loaded tool. Args passed as s-expression to Rust."
  (let ((tool (gethash tool-name *tool-registry*)))
    (unless tool (error "Tool not found: ~A" tool-name))
    ;; If it's a Forge-loaded tool, use forge_call
    ;; Otherwise, use direct CFFI calls
    (if (tool-source tool)
        ;; Forge-loaded: generic call via forge_call
        (let* ((args-str (format nil "~S" args))
               (result-ptr (forge-call (tool-handle tool) function-name args-str)))
          (unwind-protect
               (read-from-string (cffi:foreign-string-to-lisp result-ptr))
            (forge-free-string result-ptr)))
        ;; Pre-built: direct CFFI (functions defined at load time)
        (apply (get-cffi-function tool function-name) args))))
```

---

## Bazel Build

```python
# BUILD.bazel (root)

load("//bazel:sbcl.bzl", "sbcl_image", "sbcl_test")
load("//bazel:rust_dynlib.bzl", "rust_dynamic_library")

# Core — essential infrastructure (Rust binary + .so libs)
rust_binary(
    name = "phoenix",
    srcs = glob(["lib/core/phoenix/src/**/*.rs"]),
    cargo_toml = "lib/core/phoenix/Cargo.toml",
)

[rust_dynamic_library(
    name = tool,
    srcs = glob(["lib/core/{}/src/**/*.rs".format(tool)]),
    cargo_toml = "lib/core/{}/Cargo.toml".format(tool),
) for tool in [
    "ouroboros",
    "vault",
    "memory",
    "mqtt-client",
    "http",
    "s3-sync",
    "git-ops",
    "rust-forge",
    "cron-scheduler",
    "push-sns",
    "recovery",
    "browser",
    "fs",
]]

# Backends — LLM providers
+ [rust_dynamic_library(
    name = tool,
    srcs = glob(["lib/backends/{}/src/**/*.rs".format(tool)]),
    cargo_toml = "lib/backends/{}/Cargo.toml".format(tool),
) for tool in ["openrouter-backend"]
]

# Tools — optional plugins
+ [rust_dynamic_library(
    name = tool,
    srcs = glob(["lib/tools/{}/src/**/*.rs".format(tool)]),
    cargo_toml = "lib/tools/{}/Cargo.toml".format(tool),
) for tool in [
    "pgp-identity",
    "webcash-wallet",
    "social",
]]

# Build SBCL core image with all Lisp files
sbcl_image(
    name = "harmonia",
    srcs = glob(["src/**/*.lisp"]),
    deps = [
        # Core
        ":ouroboros",
        ":vault",
        ":memory",
        ":mqtt-client",
        ":http",
        ":s3-sync",
        ":git-ops",
        ":rust-forge",
        ":cron-scheduler",
        ":push-sns",
        ":recovery",
        ":browser",
        ":fs",
        # Backends
        ":openrouter-backend",
        # Tools (optional — remove if not needed)
        ":pgp-identity",
        ":webcash-wallet",
        ":social",
    ],
    main = "src/core/boot.lisp",
    quicklisp_deps = ["cffi", "bordeaux-threads", "usocket"],
    # NOTE: NO serialization libraries. NO cl-json. NO cl-mqtt.
    # Lisp only needs CFFI (to load .so) and threading.
)

sbcl_test(
    name = "harmonia-tests",
    srcs = glob(["tests/**/*.lisp"]),
    deps = [":harmonia"],
)
```

---

## Configuration Files

### `config/agent.sexp` — Agent Identity and Behavior

```lisp
;; Agent-level configuration. Read once at boot. Not modifiable by agent.
(
  :agent-id "harmonia-prime"
  :version "0.1.0"

  ;; Phoenix supervisor settings
  :phoenix (
    :heartbeat-interval-ms 5000
    :max-crash-retries 5
    :crash-cooldown-ms 60000     ; Wait 1 min between crashes before escalating
    :trauma-injection t          ; Feed crash logs back to agent on restart
    :backup-path "/var/harmonia/state/harmonia.core.bak"
    :s3-fallback-key "harmonia-state/latest/harmonia.core"
  )

  ;; Evolution engine constraints
  :evolution (
    :enabled t
    :max-rewrite-size-ratio 1.1   ; New code must be <= 110% of old code size
    :min-harmonic-score-delta 0.01 ; Rewrite must improve score by at least this
    :forbidden-files ("src/dna/constitution.lisp")  ; DNA is immutable
    :daily-rewrite-limit 20       ; Maximum self-rewrites per day
    :model-for-self-rewrite "anthropic/claude-opus-4.6"
    :model-for-code-gen "x-ai/grok-code-fast-1"
    :model-for-routine "moonshotai/kimi-k2.5"
  )

  ;; Memory constraints
  :memory (
    :max-ram-mb 512               ; Hard limit for libmemory.so
    :db-path "/var/harmonia/memory/"
    :compression-interval-hours 24
    :max-daily-entries 10000
  )

  ;; MQTT connection
  :mqtt (
    :broker-url "mqtt.harmonia.local"
    :port 1883
    :keepalive-seconds 60
    :topics-subscribe (
      "harmonia/+/cmd/#"          ; Commands from mobile apps
      "harmonia/+/sensor/#"       ; Sensor data from mobile apps
      "harmonia/system/#"         ; System events
    )
  )

  ;; Cost controls
  :cost (
    :daily-limit-usd 10.0
    :warning-threshold-usd 7.0
    :emergency-model "moonshotai/kimi-k2.5"  ; Fallback when budget exhausted
  )

  ;; Snapshot schedule
  :snapshots (
    :body-to-s3-interval-hours 6
    :dna-to-git-interval-hours 1
    :s3-bucket "harmonia-state"
    :s3-region "us-east-1"
  )
)
```

### `config/tools.sexp` — Tool Registry Configuration

```lisp
;; Lists all Rust .so tools to load at boot.
;; Each entry: (:name <symbol> :path <.so path> :category <core|backend|optional>
;;              :auto-load <t|nil> :health-check <function-name>)

(
  ;; Core tools — always loaded
  (:name "vault"          :path "lib/core/vault/libvault.so"
   :category :core :auto-load t :health-check "vault_list_keys")

  (:name "memory"         :path "lib/core/memory/libmemory.so"
   :category :core :auto-load t :health-check "memory_stats")

  (:name "mqtt-client"    :path "lib/core/mqtt-client/libmqtt.so"
   :category :core :auto-load t :health-check "mqtt_is_connected")

  (:name "http"           :path "lib/core/http/libhttp.so"
   :category :core :auto-load t :health-check nil)

  (:name "s3-sync"        :path "lib/core/s3-sync/libs3.so"
   :category :core :auto-load t :health-check nil)

  (:name "git-ops"        :path "lib/core/git-ops/libgit.so"
   :category :core :auto-load t :health-check "git_head")

  (:name "rust-forge"     :path "lib/core/rust-forge/libforge.so"
   :category :core :auto-load t :health-check nil)

  (:name "cron-scheduler" :path "lib/core/cron-scheduler/libscheduler.so"
   :category :core :auto-load t :health-check "scheduler_list")

  (:name "push-sns"       :path "lib/core/push-sns/libpush.so"
   :category :core :auto-load t :health-check nil)

  (:name "recovery"       :path "lib/core/recovery/librecovery.so"
   :category :core :auto-load t :health-check nil)

  (:name "browser"        :path "lib/core/browser/libbrowser.so"
   :category :core :auto-load t :health-check nil)

  (:name "fs"             :path "lib/core/fs/libfs.so"
   :category :core :auto-load t :health-check nil)

  (:name "ouroboros"      :path "lib/core/ouroboros/libouroboros.so"
   :category :core :auto-load t :health-check "ouroboros_health")

  ;; Backend — LLM provider
  (:name "openrouter"     :path "lib/backends/openrouter-backend/libharmonia_openrouter.so"
   :category :backend :auto-load t :health-check "or_model_count")

  ;; Optional tools — loaded on demand
  (:name "pgp-identity"   :path "lib/tools/pgp-identity/libpgp.so"
   :category :optional :auto-load nil :health-check nil)

  (:name "webcash-wallet" :path "lib/tools/webcash-wallet/libwebcash.so"
   :category :optional :auto-load nil :health-check nil)

  (:name "social"         :path "lib/tools/social/libsocial.so"
   :category :optional :auto-load nil :health-check nil)
)
```

### `config/backends.sexp` — Backend Provider Configuration

```lisp
;; Backend-specific configuration.
;; Each backend .so reads its own config section at init time.

(
  (:name "openrouter"
   :so-path "lib/backends/openrouter-backend/libharmonia_openrouter.so"
   :env-key "OPENROUTER_API_KEY"    ; Vault reads from this env var
   :base-url "https://openrouter.ai/api/v1"
   :daily-limit-usd 10.0
   :models (
     ;; Tier 1: Speed/Tooling
     (:id "moonshotai/kimi-k2.5"
      :use-for (:code-generation :simple-logic :data-generation)
      :max-tokens 4096
      :cost-per-1k-prompt 0.0003
      :cost-per-1k-completion 0.0006)
     ;; Tier 2: Logic/Refactor
     (:id "x-ai/grok-code-fast-1"
      :use-for (:refactoring :optimization :tool-generation)
      :max-tokens 8192
      :cost-per-1k-prompt 0.001
      :cost-per-1k-completion 0.002)
     ;; Tier 3: Deep Wisdom
     (:id "anthropic/claude-opus-4.6"
      :use-for (:self-rewrite :architecture :critical-reasoning)
      :max-tokens 16384
      :cost-per-1k-prompt 0.015
      :cost-per-1k-completion 0.075)
   ))
)
```

---

## Vault Configuration (Tool-Key Bindings)

The Vault's permission configuration is **static and non-writable by the agent**. It is deployed at infrastructure provisioning time and lives outside the agent's sandbox.

### `/etc/harmonia/vault.toml`

```toml
# Vault master configuration.
# This file is NEVER accessible to the agent's Lisp code.
# It is read only by libvault.so at initialization.

[master]
keychain_path = "/var/harmonia/vault/keychain.enc"
# Master key is read from VAULT_MASTER_KEY environment variable.
# It is set at deploy time via AMI bake or Lambda configuration.

# Tool-Key Bindings
# Format: key_symbol -> list of .so files authorized to access it.
# ANY request from a .so NOT in this list is DENIED.

[[bindings]]
key = "openrouter"
env_source = "OPENROUTER_API_KEY"
allowed_callers = [
    "lib/backends/openrouter-backend/libharmonia_openrouter.so",
]

[[bindings]]
key = "aws_s3"
env_source = "AWS_ACCESS_KEY_ID"  # Also reads AWS_SECRET_ACCESS_KEY
allowed_callers = [
    "lib/core/s3-sync/libs3.so",
]

[[bindings]]
key = "mqtt_credentials"
env_source = "MQTT_PASSWORD"
allowed_callers = [
    "lib/core/mqtt-client/libmqtt.so",
]

[[bindings]]
key = "git_deploy_key"
env_source = "GIT_SSH_KEY_PATH"
allowed_callers = [
    "lib/core/git-ops/libgit.so",
]

[[bindings]]
key = "push_credentials"
env_source = "AWS_SNS_ACCESS_KEY"
allowed_callers = [
    "lib/core/push-sns/libpush.so",
]

[[bindings]]
key = "webcash_seed"
env_source = "WEBCASH_WALLET_SEED"
allowed_callers = [
    "lib/tools/webcash-wallet/libwebcash.so",
]

# Example: WireGuard VPN key (future)
# [[bindings]]
# key = "wg_private"
# env_source = "WG_PRIVATE_KEY"
# allowed_callers = [
#     "lib/tools/wireguard/libwireguard.so",
# ]
```

### How Permission Checking Works

```rust
// vault/src/permissions.rs

/// Check if the calling .so is authorized to access a key.
/// Uses the return address to determine which .so made the call.
fn check_caller_permission(key_symbol: &str, caller_so_path: &str) -> bool {
    let config = VAULT_CONFIG.read().unwrap();
    if let Some(binding) = config.bindings.iter().find(|b| b.key == key_symbol) {
        binding.allowed_callers.iter().any(|allowed| {
            // Normalize paths and compare
            std::path::Path::new(caller_so_path)
                .canonicalize()
                .map(|p| {
                    binding.allowed_callers.iter().any(|a| {
                        std::path::Path::new(a).canonicalize()
                            .map(|ap| ap == p)
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
    } else {
        false  // Unknown key — deny by default
    }
}
```

**Security invariant:** Even if the agent rewrites its entire Lisp codebase, it cannot alter `vault.toml` (outside its sandbox), cannot forge a caller identity (verified by Rust at the binary level), and cannot access any secret it is not bound to.

---

## Bootstrap Sequence

```bash
#!/bin/bash
# scripts/bootstrap.sh

set -e

echo "=== Harmonia Bootstrap ==="

# 1. Install SBCL
if ! command -v sbcl &> /dev/null; then
    echo "Installing SBCL..."
    # NetBSD: pkgin install sbcl
    # Linux: apt install sbcl
    pkgin install sbcl || apt install -y sbcl
fi

# 2. Install Quicklisp (only for CFFI and bordeaux-threads)
if [ ! -f ~/quicklisp/setup.lisp ]; then
    echo "Installing Quicklisp..."
    curl -o /tmp/quicklisp.lisp https://beta.quicklisp.org/quicklisp.lisp
    sbcl --load /tmp/quicklisp.lisp \
         --eval '(quicklisp-quickstart:install)' \
         --eval '(quit)'
fi

# 3. Install Rust (for tool compilation)
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
fi

# 4. Build all Rust lib crates with Bazel
echo "Building Harmonia lib..."
bazel build //harmonia/lib/...

# 5. Set up tool library path
export HARMONIA_LIB_PATH=$(bazel info bazel-bin)/harmonia/lib
export LD_LIBRARY_PATH=$HARMONIA_LIB_PATH:$LD_LIBRARY_PATH

# 6. Boot Harmonia
echo "Starting Harmonia..."
sbcl --load src/core/boot.lisp \
     --eval '(harmonia:start)'
```

---

## What Harmonia Learns to Do

Over time, through evolution, Harmonia discovers:

1. **Optimal memory format** — Maybe s-expressions stored as flat files are better for simple recall. Maybe a graph structure for relational memory. The benchmarks decide.

2. **Optimal model routing** — kimi-k2.5 for code tasks, grok-code-fast-1 for quick answers, Claude Opus only for complex reasoning about self-modification.

3. **Tool composition patterns** — "When user asks about travel, fire location + calendar + maps + weather simultaneously via their respective Rust `.so` tools, then compose results in Lisp."

4. **New tools it needs** — "I keep getting asked about emails but have no email tool. Let me write one via the Forge." The agent generates Rust source, compiles it, loads it, tests it — all autonomously.

5. **Harmonic patterns in user behavior** — "User always checks news at 8am, schedules meetings at 10am, has lunch at 12:30pm. Pre-fetch relevant info 5 minutes before each." Implemented by setting cron entries via the cron-scheduler `.so`.

6. **Self-improvement strategies** — Which parts of its code benefit most from rewriting. Where the harmonic score plateaus. When to stop optimizing and start exploring.

7. **New backends** — The agent can use the Forge to write, compile, and load new Rust backend `.so` libraries for additional LLM providers, vector databases, or any external service.

---

## Quicklisp Dependencies (Minimal)

The only Quicklisp packages needed:

| Package | Purpose |
|---------|---------|
| `cffi` | Load Rust `.so` dynamic libraries |
| `bordeaux-threads` | Threading for concurrent tool calls |
| `usocket` | Only if needed for local IPC (may be eliminated) |

That is it. No `cl-json`. No `cl-mqtt`. No `dexador`. No `jonathan`. No serialization libraries. Lisp stays pure.
