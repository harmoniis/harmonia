# harmonia-recovery

## Purpose

Watchdog and crash capture subsystem. Records structured recovery events (crashes, restarts, anomalies) to a persistent log and provides tail access for monitoring.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_recovery_version` | `() -> *const c_char` | Version string |
| `harmonia_recovery_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_recovery_record` | `(kind: *const c_char, detail: *const c_char) -> i32` | Append recovery event |
| `harmonia_recovery_tail_lines` | `(limit: i32) -> *mut c_char` | Read last N lines from log |
| `harmonia_recovery_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_recovery_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_RECOVERY_LOG` | `$STATE_ROOT/recovery.log` | Recovery event log path |

## Log Format

Each line: `<unix_timestamp>\t<kind>\t<detail>`

Example:
```
1709654321	tool/crash	search-exa: connection refused
1709654400	phoenix/restart	child-exit=1 stderr=timeout
```

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_recovery_record"
  :string "tool/crash" :string "search-exa: connection refused" :int)
(let ((tail (cffi:foreign-funcall "harmonia_recovery_tail_lines" :int 20 :pointer)))
  (cffi:foreign-string-to-lisp tail))
```

## Self-Improvement Notes

- Shared log file with `ouroboros` and `phoenix`; all three write to `recovery.log`.
- `tail_lines` reads the full file and returns the last N lines; optimize for large logs with seek.
- `record` creates parent directories automatically (safe for first-run).
- To add structured queries: migrate to SQLite or parse the TSV into an in-memory index.
