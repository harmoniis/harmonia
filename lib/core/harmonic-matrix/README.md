# harmonia-harmonic-matrix

## Purpose

Constrained route mesh that governs which tools/agents can communicate with each other. Tracks node registration, edge permissions, route observations, and time-series metrics for the entire agent topology.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_harmonic_matrix_version` | `() -> *const c_char` | Version string |
| `harmonia_harmonic_matrix_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_harmonic_matrix_init` | `() -> i32` | Initialize matrix state |
| `harmonia_harmonic_matrix_set_store` | `(kind: *const c_char, dsn: *const c_char) -> i32` | Set backing store (sqlite/graph) |
| `harmonia_harmonic_matrix_get_store` | `() -> *mut c_char` | Current store config |
| `harmonia_harmonic_matrix_register_node` | `(name: *const c_char, kind: *const c_char) -> i32` | Register a node (tool/agent/frontend) |
| `harmonia_harmonic_matrix_set_tool_enabled` | `(name: *const c_char, enabled: i32) -> i32` | Enable/disable a tool node |
| `harmonia_harmonic_matrix_register_edge` | `(from: *const c_char, to: *const c_char, label: *const c_char) -> i32` | Register a permitted route |
| `harmonia_harmonic_matrix_route_allowed` | `(from: *const c_char, to: *const c_char) -> i32` | Check if route is permitted (1=yes) |
| `harmonia_harmonic_matrix_observe_route` | `(from: *const c_char, to: *const c_char, latency_ms: i64) -> i32` | Record a route observation |
| `harmonia_harmonic_matrix_log_event` | `(from: *const c_char, to: *const c_char, kind: *const c_char, detail: *const c_char) -> i32` | Log an event on a route |
| `harmonia_harmonic_matrix_route_timeseries` | `(from: *const c_char, to: *const c_char, limit: i32) -> *mut c_char` | Get time-series for a route |
| `harmonia_harmonic_matrix_time_report` | `(since_unix: u64) -> *mut c_char` | Time report since timestamp |
| `harmonia_harmonic_matrix_report` | `() -> *mut c_char` | Full matrix report |
| `harmonia_harmonic_matrix_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_harmonic_matrix_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_MATRIX_STORE_KIND` | `memory` | Store backend: `memory`, `sqlite`, `graph` |
| `HARMONIA_MATRIX_DB` | `$STATE_ROOT/matrix.db` | SQLite path (when kind=sqlite) |
| `HARMONIA_MATRIX_GRAPH_URI` | -- | Graph DB URI (when kind=graph) |
| `HARMONIA_MATRIX_HISTORY_LIMIT` | `1000` | Max time-series entries per route |

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_harmonic_matrix_init" :int)
(cffi:foreign-funcall "harmonia_harmonic_matrix_register_node"
  :string "search-exa" :string "tool" :int)
(cffi:foreign-funcall "harmonia_harmonic_matrix_register_edge"
  :string "lisp-core" :string "search-exa" :string "invoke" :int)
(cffi:foreign-funcall "harmonia_harmonic_matrix_route_allowed"
  :string "lisp-core" :string "search-exa" :int) ;; => 1
```

## Self-Improvement Notes

- Three store backends: in-memory (default), SQLite (persistent), graph DB (external).
- The matrix enforces that only registered edges are traversable; unauthorized routes return 0.
- Time-series observations enable the agent to learn which routes are slow and optimize.
- `report()` returns the full topology as an S-expression; useful for self-reflection.
- To add weighted routing: extend edge metadata with cost/priority fields.
