# harmonia-ouroboros

## Purpose

Self-healing subsystem implementing the crash-reflect-patch-reload cycle. Records crash events, maintains crash history, and writes patch files that the agent can apply to fix itself.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_ouroboros_version` | `() -> *const c_char` | Version string |
| `harmonia_ouroboros_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_ouroboros_record_crash` | `(kind: *const c_char, detail: *const c_char) -> i32` | Record a crash event |
| `harmonia_ouroboros_last_crash` | `() -> *mut c_char` | Get most recent crash record |
| `harmonia_ouroboros_history` | `(limit: i32) -> *mut c_char` | Get crash history (S-expression) |
| `harmonia_ouroboros_write_patch` | `(filename: *const c_char, content: *const c_char) -> i32` | Write a patch file to patch dir |
| `harmonia_ouroboros_health` | `() -> *mut c_char` | Health summary |
| `harmonia_ouroboros_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_ouroboros_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_RECOVERY_LOG` | `$STATE_ROOT/recovery.log` | Shared recovery log |
| `HARMONIA_OUROBOROS_PATCH_DIR` | `$STATE_ROOT/patches` | Directory for generated patches |

## Usage from Lisp

```lisp
;; After a crash, record and reflect
(cffi:foreign-funcall "harmonia_ouroboros_record_crash"
  :string "tool/search-exa" :string "timeout after 30s" :int)
;; Generate a fix
(cffi:foreign-funcall "harmonia_ouroboros_write_patch"
  :string "fix-exa-timeout.patch" :string "diff content..." :int)
;; Review history for patterns
(cffi:foreign-funcall "harmonia_ouroboros_history" :int 10 :pointer)
```

## Self-Improvement Notes

- Crash records are appended to `recovery.log` in TSV format: `<unix_ts>\t<kind>\t<detail>`.
- `history()` parses recovery.log and returns the last N entries as an S-expression list.
- Patches are written to the patch directory; the Lisp orchestrator decides when to apply them.
- The crash-reflect-patch loop: (1) record_crash -> (2) LLM analyzes history -> (3) write_patch -> (4) rust-forge compiles -> (5) gateway hot-reloads.
- To add automatic patch application: integrate with `rust-forge` and `gateway` for compile-and-reload.
