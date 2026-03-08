# harmonia-config-store

## Purpose

Runtime-mutable configuration key-value store backed by SQLite. Provides scoped configuration namespaces so different subsystems can store and retrieve settings without collision.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_config_store_version` | `() -> *const c_char` | Version string |
| `harmonia_config_store_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_config_store_init` | `() -> i32` | Initialize/open SQLite DB |
| `harmonia_config_store_set` | `(scope: *const c_char, key: *const c_char, value: *const c_char) -> i32` | Set scoped key-value |
| `harmonia_config_store_get` | `(scope: *const c_char, key: *const c_char, default_val: *const c_char) -> *mut c_char` | Get value with default |
| `harmonia_config_store_list` | `(scope: *const c_char) -> *mut c_char` | List all keys in scope |
| `harmonia_config_store_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_config_store_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_CONFIG_DB` | `$STATE_ROOT/config.db` | SQLite database path |

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_config_store_init" :int)
(cffi:foreign-funcall "harmonia_config_store_set"
  :string "elevenlabs" :string "default_voice" :string "rachel" :int)
(cffi:foreign-funcall "harmonia_config_store_get"
  :string "elevenlabs" :string "default_voice" :string "rachel" :pointer)
(cffi:foreign-funcall "harmonia_config_store_list" :string "elevenlabs" :pointer)
```

## Self-Improvement Notes

- Schema: `(scope TEXT, key TEXT, value TEXT, PRIMARY KEY (scope, key))`.
- `get` accepts a default value pointer (can be null) returned when key is missing.
- `list` returns an S-expression alist: `((key1 . val1) (key2 . val2) ...)`.
- Used by tools (e.g., elevenlabs default voice) and frontends for runtime preferences.
- To add change notifications: store a generation counter and expose a `changed_since(gen)` FFI.
