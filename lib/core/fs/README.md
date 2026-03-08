# harmonia-fs

## Purpose

Sandboxed filesystem I/O restricted to a configurable root directory. Prevents the agent from reading or writing outside its designated workspace.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_fs_version` | `() -> *const c_char` | Version string |
| `harmonia_fs_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_fs_write` | `(path: *const c_char, content: *const c_char) -> i32` | Write file (must be under root) |
| `harmonia_fs_read` | `(path: *const c_char) -> *mut c_char` | Read file contents |
| `harmonia_fs_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_fs_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |
| `HARMONIA_FS_ROOT` | `$STATE_ROOT/fs` | Sandbox root directory |

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_fs_write"
  :string "notes/todo.txt" :string "fix search timeout" :int)
(let ((content (cffi:foreign-funcall "harmonia_fs_read"
                 :string "notes/todo.txt" :pointer)))
  (cffi:foreign-string-to-lisp content))
```

## Self-Improvement Notes

- Paths are canonicalized and checked against the sandbox root; traversal attacks (`../`) are rejected.
- Parent directories are created automatically on write.
- To add directory listing: implement `harmonia_fs_list(dir)` returning S-expression file list.
- To add delete/move: follow the same sandbox-check pattern before any destructive operation.
- Binary file support: currently text-only via C strings; use base64 encoding for binary data.
