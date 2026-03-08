# harmonia-tailnet

## Purpose

Tailscale mesh networking layer providing peer discovery and TCP-based signal transport between Harmonia nodes on a shared tailnet.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_tailnet_version` | `() -> *const c_char` | Version string |
| `harmonia_tailnet_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_tailnet_init` | `(config: *const c_char) -> i32` | Initialize with S-expression config |
| `harmonia_tailnet_discover_peers` | `() -> *mut c_char` | Discover tailnet peers (S-expression list) |
| `harmonia_tailnet_send` | `(peer: *const c_char, channel: *const c_char, payload: *const c_char) -> i32` | Send signal to a peer |
| `harmonia_tailnet_poll` | `() -> *mut c_char` | Poll for inbound signals from peers |
| `harmonia_tailnet_node_info` | `() -> *mut c_char` | This node's identity/info |
| `harmonia_tailnet_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_tailnet_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_TAILNET_PORT` | `9377` | TCP listen port |
| `HARMONIA_TAILNET_HOSTNAME_PREFIX` | `harmonia-` | Prefix for peer discovery |
| `HOSTNAME` / `HOST` | system hostname | This node's hostname |

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_tailnet_init" :string "()" :int)
(let ((peers (cffi:foreign-funcall "harmonia_tailnet_discover_peers" :pointer)))
  (format t "Peers: ~a~%" (cffi:foreign-string-to-lisp peers)))
(cffi:foreign-funcall "harmonia_tailnet_send"
  :string "node-2" :string "sync" :string "ping" :int)
```

## Self-Improvement Notes

- Uses raw TCP sockets (std::net) with a background listener thread.
- Peer discovery is hostname-based: scans `HARMONIA_TAILNET_HOSTNAME_PREFIX*` names on the tailnet.
- Messages are newline-delimited JSON over TCP.
- To add encryption: wrap the TCP stream with rustls or use Tailscale's built-in WireGuard.
- The mesh module (`mesh.rs`) handles connection pooling and reconnection.
