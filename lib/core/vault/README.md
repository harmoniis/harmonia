# harmonia-vault

## Purpose

Wallet-rooted secret store with scoped symbol-based access. Secrets are referenced by symbolic names (e.g., `openrouter`, `exa_api_key`) so raw credentials do not appear in agent code or logs.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_vault_version` | `() -> *const c_char` | Version string |
| `harmonia_vault_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_vault_init` | `() -> i32` | Initialize vault from env/db |
| `harmonia_vault_get_secret` | `(symbol: *const c_char) -> *mut c_char` | Retrieve secret by symbol |
| `harmonia_vault_has_secret` | `(symbol: *const c_char) -> i32` | Check if symbol exists (1=yes, 0=no) |
| `harmonia_vault_list_symbols` | `() -> *mut c_char` | List all available symbols |
| `harmonia_vault_set_secret` | `(symbol: *const c_char, value: *const c_char) -> i32` | Store a secret |
| `harmonia_vault_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_vault_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_VAULT_DB` | `$STATE_ROOT/vault.db` | SQLite database path |
| `HARMONIA_VAULT_WALLET_DB` | `~/.harmoniis/master.db` | Harmoniis wallet master DB used to resolve vault root slot |
| `HARMONIA_VAULT_SLOT_FAMILY` | `vault` | Wallet slot family to derive vault root key material |
| `HARMONIA_VAULT_MASTER_KEY` | -- | Optional fallback key material (hex/raw), HKDF-derived to 32 bytes only when wallet slot root is unavailable |
| `HARMONIA_VAULT_ALLOW_UNENCRYPTED` | `false` | Emergency compatibility fallback; when `false`, writes fail if no vault key is available |
| `HARMONIA_VAULT_STORE` | (legacy) | Legacy flat-file vault path |
| `HARMONIA_VAULT_SECRET__<SYMBOL>` | -- | Inject secret via env var (e.g., `HARMONIA_VAULT_SECRET__openrouter=sk-...`) |
| `HARMONIA_VAULT_IMPORT` | -- | Comma-separated env var names to import as symbols |
| `HARMONIA_VAULT_COMPONENT_POLICY` | -- | Optional component->symbol pattern policy overrides (`component=a,b*;other=*`) |

## Rust API (used by other crates)

```rust
use harmonia_vault::{init_from_env, get_secret_for_component, has_secret_for_symbol};
init_from_env().unwrap();
let key = get_secret_for_component("openrouter-backend", "openrouter")
    .unwrap()
    .unwrap();
```

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_vault_init" :int)
(cffi:foreign-funcall "harmonia_vault_has_secret" :string "openrouter" :int)
;; Never log the return value of get_secret!
```

## Self-Improvement Notes

- Backed by SQLite (`rusqlite`). The `store.rs` module handles DB init and CRUD.
- `ingest.rs` scans env vars with `HARMONIA_VAULT_SECRET__` prefix on init.
- Symbol names are case-insensitive and colon-prefix-tolerant (`:OpenRouter` -> `openrouter`).
- Writes are encrypted at rest with AES-256-GCM (`aead:v1` records) using key material rooted in the wallet vault slot family (`vault` by default). `HARMONIA_VAULT_MASTER_KEY` is fallback-only.
- Randomness is used only for per-record AEAD nonces; root/master key material remains deterministic from wallet slot derivation.
- Legacy XOR-obfuscated (`enc:`) values are read for migration compatibility.
- Component-scoped reads are enforced through `get_secret_for_component`.
- A deterministic scoped derivation API (`derive_component_seed_hex`) supports recoverable child material (e.g., MQTT TLS lineage).
