# harmonia-tui

## Purpose

Terminal UI frontend channel. Reads from stdin and writes to stdout, providing a direct interactive interface for local development and debugging.

## Channel Format

- Channel name: `tui`
- Sub-channel: `tui:console` (single channel)
- Security label: `local` (no network, no auth required)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with S-expression config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for input (non-blocking) |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send output to channel |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

No vault symbols required. Config S-expression: `()` (empty).

## Self-Improvement Notes

- Implements the standard 7-function frontend FFI contract.
- `poll` reads from a background stdin reader thread; returns 0 if no input, >0 with bytes written to buf.
- `send` writes to stdout with a newline.
- Compiled as cdylib, hot-loaded by gateway via `harmonia_gateway_register`.
- To add rich TUI: integrate `ratatui` or `crossterm` for formatted output.
