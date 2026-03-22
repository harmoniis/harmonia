# Harmonia v0.1.9 — Changelog

## Architecture: Actor System Redesign

### Rust Runtime (ractor actors)
- **Every component has an actor** — vault, config, provider-router, parallel (previously bare dispatch functions) now have dedicated ractor actors. The supervisor ROUTES messages, never EXECUTES dispatch logic.
- **Non-blocking supervisor** — `ComponentCall` dispatched to component actors via `call_t!(actor, Dispatch)`. LLM calls, database queries, and HTTP requests no longer block heartbeats, drains, or shutdown.
- **Respawn limits** — max 5 respawns per actor with counter tracking. Prevents infinite crash loops that previously consumed CPU.
- **Coordinated shutdown** — 2s component grace period, then supervisor stop, then 5s hard exit via `process::exit`. No more hanging processes requiring SIGKILL.
- **Matrix error logging** — all matrix operations log errors instead of silently discarding `Result` values.

### Lisp Runtime (actor system inspired by cl-gserver/Sento)
- **`actors.lisp`** — lightweight actor system using SBCL threads + `sb-concurrency` mailboxes. Provides `make-actor`, `tell`, `ask`, `actor-reply`, `stop-actor`, `start-timer`, and `actor-system` with coordinated shutdown.
- **5 concurrent actors** replace the sequential tick loop: gateway, swarm, conductor, harmonic, chronicle. Each owns its state, processes `:tick` messages independently.
- **Per-thread IPC connections** — thread-safe connection pool prevents socket corruption when multiple actors call IPC simultaneously.
- **Thread-safe queues** — prompt queue and outbound gateway queue use `sb-thread:with-mutex` for safe cross-actor access.
- **30s IPC deadline** — `sb-sys:with-deadline` in `ipc-call` prevents the tick loop from blocking forever on a stuck runtime call.
- **Sequential mode preserved** — `run-loop` with `max-cycles` still runs the sequential tick for deterministic testing.

## Module Registry System

- **`registry.rs`** — `ModuleEntry` with name, status, core flag, config requirements (`VaultSecret`/`ConfigKey`), init/shutdown functions. Single source of truth for all 22 modules.
- **`harmonia modules` CLI** — list, load, unload, reload modules via runtime IPC. Shows loaded/unloaded/error status with missing config details.
- **Config validation** — vault secrets and config-store keys checked at startup. Policy-denied reads skipped gracefully (module init validates internally).
- **Auto-enable** — `setup.rs` detects configured modules after setup and persists the enabled list to config-store.

## Install System

- **`--config` flag** for `install.sh` — headless provisioning from JSON file. `HARMONIA_CONFIG_JSON` env var also supported.
- **`harmonia setup --headless-config`** — provisions vault secrets, config-store values, paths, and auto-detects enabled modules.
- **`config/install-config.template.json`** — comprehensive template with 30 vault secret keys, config-store values, model policy, frontend configs.

## TUI Session System

- **New session per invocation** — `harmonia` always starts a clean session. No stale history from previous runs.
- **`/resume`** — pick a past session, conversation history replays in the TUI with the same ╭─/│/╰─ format.
- **`/rewind`** — rewind conversation to any previous turn (like `git reset`). Events file truncated, screen clears, history replays up to chosen turn.
- **`/clear`** — create a fresh session mid-conversation, clear screen.
- **Response buffer** — reader thread buffers response lines in `Arc<Mutex<Vec<String>>>`, main thread renders after spinner cleanup. No more cursor fight between threads.
- **`ExitReason` enum** — `UserQuit`, `CtrlC`, `ConnectionLost`, `Error`. `/exit` shows "Goodbye.", connection loss shows diagnostic with `harmonia status` suggestion.
- **Input undo/redo** — Ctrl+Z undoes last text change, Ctrl+Y redoes. Snapshot stack saves `(text, cursor)` before every mutation.
- **Command palette** — type `/` and filter commands by substring match (command name + description). Right-aligned help text. Arrow keys to select, Enter/Tab to accept.

## Terminal Reset

- **`reset_terminal_if_needed()`** — runs at start of every `main()`. Uses raw `libc::tcgetattr`/`tcsetattr` to check if OPOST is off (raw mode from crashed TUI) and restores cooked mode. Fixes the staircase output pattern that `crossterm::disable_raw_mode()` cannot fix across processes.

## Orchestration Fixes

- **Owner signal pass-through** — TUI prompts pass raw user text to the LLM, no `[BASEBAND CHANNEL]` envelope wrapping. The user IS the owner, not an external data source.
- **Capable model for owner prompts** — direct answers use the best available seed model instead of the cheap `inception/mercury-2`.
- **Prompt format** — user question placed first, system context labeled "for reference only". Prevents LLM from treating context as the task.
- **Phoenix health restored** — `hfetch` (removed CFFI function) replaced with raw `sb-bsd-sockets` HTTP GET using proper HTTP/1.1 + CRLF.

## Status Command

- **Rich display** — uptime, subsystem health (color-coded), module summary (loaded count + unconfigured list with missing config details), paths, helpful commands.

## CI / Release

- **FreeBSD native VM build** — replaces broken `cross` compilation with `cross-platform-actions/action@v0.32.0`. Boots real FreeBSD 14.2 via QEMU, installs Rust natively, builds and tests. No more `rustfmt not found` errors.
- **FreeBSD in Build+Test** — FreeBSD now runs in the test matrix alongside macOS, Ubuntu, Windows.
- **Release platforms** — linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64, windows-x86_64, freebsd-x86_64.

## Tests

- **125 Rust tests** — all workspace crates including 8 EditBuffer unit tests, 16 harmonia CLI tests.
- **30 Lisp actor tests** — actor creation, ask/tell, state management, error handling, actor system, timer, thread-safe queue, outbound queue, IPC.
- **16 install config tests** — template validation, structure, vault keys, config store, install.sh syntax.
- **Test scripts** — `test-module-registry.sh`, `test-lisp-actors.lisp`, `test-install-config.sh`, `test-terminal-reset.sh`.
