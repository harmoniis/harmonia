# harmonia-config-store

## Purpose

Runtime-mutable configuration key-value store backed by SQLite. Provides scoped configuration namespaces so different subsystems can store and retrieve settings without collision. Policy-gated component access control prevents cross-scope reads/writes.

## Architecture

```
Lookup chain:  cache → DB → registry-derived env var → default
```

Env var names are derived from `(scope, key)` pairs by three rules:
1. **Global scope** drops its name: `("global", "state-root")` → `HARMONIA_STATE_ROOT`
2. **Suffix stripping**: `-backend`, `-frontend`, `-tool`, `-core`, `-storage` stripped from scope
3. **Stem aliases**: `harmonic-matrix` → `matrix`, `search-exa` → `exa`, etc.

The ~5 entries that break all rules carry an explicit `env_override` in the registry.

All known entries are declared in `src/registry.rs` — the single source of truth for config entry names.

## FFI Surface

### Simple API (no policy enforcement)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_config_store_init` | `() -> i32` | Initialize DB, load cache, seed from env |
| `harmonia_config_store_set` | `(scope, key, value) -> i32` | Set scoped key-value |
| `harmonia_config_store_get` | `(scope, key) -> *mut c_char` | Get value (null if missing) |
| `harmonia_config_store_list` | `(scope) -> *mut c_char` | List keys in scope (newline-separated) |
| `harmonia_config_store_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_config_store_free_string` | `(ptr)` | Free returned strings |
| `harmonia_config_store_version` | `() -> *const c_char` | Version string |
| `harmonia_config_store_healthcheck` | `() -> i32` | Returns 1 |

### Policy-Gated API (component-aware)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_config_store_get_for` | `(component, scope, key) -> *mut c_char` | Policy-checked get |
| `harmonia_config_store_get_or` | `(component, scope, key, default) -> *mut c_char` | Policy-checked get with default |
| `harmonia_config_store_set_for` | `(component, scope, key, value) -> i32` | Policy-checked set |
| `harmonia_config_store_delete_for` | `(component, scope, key) -> i32` | Admin-only delete |
| `harmonia_config_store_dump` | `(component, scope) -> *mut c_char` | Dump scope as `key=value\n` |
| `harmonia_config_store_ingest_env` | `() -> i32` | Seed DB from env vars (first-run) |

## Policy

- **Admin** components (`conductor`, `admin-intent`, `harmonia-cli`): full read/write/delete
- **Component**: can read `global` scope + own scope; can write own scope only
- **Override**: `HARMONIA_CONFIG_STORE_POLICY` env var (`comp=scope1,scope2:rw;comp2=scope3:r`)

## Bootstrap

`HARMONIA_STATE_ROOT` and `HARMONIA_CONFIG_DB` are read directly from env before the DB is available (chicken-and-egg). All other config flows through the store.

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_CONFIG_DB` | `$STATE_ROOT/config.db` | SQLite database path |

## Rust API

```rust
// Initialize (call once at startup)
harmonia_config_store::init_v2()?;

// Read from own scope
let url = harmonia_config_store::get_own_or("openai-backend", "base-url", "https://api.openai.com/v1")?;

// Read from another scope (policy-checked)
let root = harmonia_config_store::get_config("my-component", "global", "state-root")?;

// Write to own scope
harmonia_config_store::set_config("openai-backend", "openai-backend", "base-url", "https://custom.api")?;
```

## Lisp API

```lisp
;; Policy-gated (preferred)
(config-get-for "openai-backend" "base-url")
(config-get-or "openai-backend" "base-url" "https://api.openai.com/v1")
(config-set-for "openai-backend" "base-url" "https://custom.api")

;; Simple (admin-level, no policy check)
(config-get "state-root" "global")
(config-set "state-root" "/custom/path" "global")
```
