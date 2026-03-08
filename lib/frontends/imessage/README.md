# harmonia-imessage

## Purpose

iMessage frontend via BlueBubbles server. Polls for incoming messages and sends replies through the BlueBubbles HTTP API, enabling the agent to communicate via Apple's iMessage platform.

## Channel Format

- Channel name: `imessage`
- Sub-channel: `imessage:<phone_or_email>` (recipient identifier)
- Security label: `authenticated` (requires BlueBubbles server + password)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for new messages |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send message to channel |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Config S-expression or env vars:
| Source | Key | Description |
|--------|-----|-------------|
| Config | `:server-url` | BlueBubbles server URL |
| Env | `HARMONIA_IMESSAGE_SERVER_URL` | Fallback server URL |
| Config | `:password` | BlueBubbles password |
| Env | `HARMONIA_IMESSAGE_PASSWORD` | Fallback password |

## Self-Improvement Notes

- Client module (`client.rs`) handles BlueBubbles HTTP API integration.
- Polls `/api/v1/message` for new messages with pagination.
- Sends via `POST /api/v1/message/text` with recipient and body.
- To add media: use BlueBubbles attachment API endpoints.
- To add group chat: parse group identifiers from incoming messages.
