# harmonia-tailscale (Frontend)

## Purpose

Inter-node signal channel over Tailscale mesh. Enables Harmonia nodes on the same tailnet to exchange messages as a frontend channel registered with the gateway.

## Channel Format

- Channel name: `tailscale`
- Sub-channel: `tailscale:<peer_hostname>` (target node)
- Security label: `mesh` (Tailscale WireGuard encrypted)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for inbound signals |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send signal to peer |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Delegates to `harmonia-tailnet` core crate for connection management. See tailnet README for env vars.

## Self-Improvement Notes

- This is the frontend wrapper around `harmonia-tailnet` core crate.
- Implements the standard frontend FFI contract so it can be gateway-registered like any other frontend.
- Used for multi-node agent coordination (task delegation, memory sync, etc.).
- To add broadcast: send to all discovered peers instead of a single target.
