# harmonia-git-ops

## Purpose

Git operations for DNA sync and self-versioning. Commits all changes in a repository and pushes to a remote, enabling the agent to version its own code and configuration.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_git_ops_version` | `() -> *const c_char` | Version string |
| `harmonia_git_ops_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_git_ops_commit_all` | `(repo_path: *const c_char, message: *const c_char) -> i32` | Stage all + commit |
| `harmonia_git_ops_push` | `(repo_path: *const c_char, remote: *const c_char) -> i32` | Push to remote |
| `harmonia_git_ops_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_git_ops_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Requires `git` on `$PATH`. No special env vars. Git credentials must be pre-configured (SSH keys or credential helper).

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_git_ops_commit_all"
  :string "/home/harmonia/agent" :string "self: apply ouroboros patch" :int)
(cffi:foreign-funcall "harmonia_git_ops_push"
  :string "/home/harmonia/agent" :string "origin" :int)
```

## Self-Improvement Notes

- `commit_all` runs `git add -A && git commit -m <msg>` in the given repo path.
- `push` runs `git push <remote>` (defaults to current branch).
- Used by the ouroboros cycle to version patches after self-modification.
- To add branch management: implement `create_branch`, `checkout`, `merge` FFI exports.
- To add diff inspection: implement `harmonia_git_ops_diff()` for pre-commit review.
