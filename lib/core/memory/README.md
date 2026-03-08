# harmonia-memory

## Purpose

Key-value memory store backed by SQLite, providing persistent storage for agent memory (facts, context, embeddings). Serves as the foundation for vector/graph DB capabilities.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_memory_version` | `() -> *const c_char` | Version string |
| `harmonia_memory_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_memory_init` | `(file_path: *const c_char) -> i32` | Open/create DB at path |
| `harmonia_memory_put` | `(key: *const c_char, value: *const c_char) -> i32` | Upsert key-value pair |
| `harmonia_memory_get` | `(key: *const c_char) -> *mut c_char` | Retrieve value by key |
| `harmonia_memory_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_memory_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |

The DB file path is passed explicitly to `init`. Convention: `$HARMONIA_STATE_ROOT/memory.db`.

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_memory_init"
  :string "/tmp/harmonia/memory.db" :int)
(cffi:foreign-funcall "harmonia_memory_put"
  :string "user:name" :string "George" :int)
(let ((val (cffi:foreign-funcall "harmonia_memory_get" :string "user:name" :pointer)))
  (cffi:foreign-string-to-lisp val))
```

## Self-Improvement Notes

- Simple KV over SQLite with `INSERT OR REPLACE`. Schema: `(key TEXT PRIMARY KEY, value TEXT)`.
- To add vector search: add an `embedding BLOB` column and implement cosine similarity in SQL or Rust.
- To add graph edges: create a second table `(from TEXT, relation TEXT, to TEXT)` with indexes.
- The `OnceLock<RwLock<Connection>>` pattern ensures thread safety for the global DB handle.
