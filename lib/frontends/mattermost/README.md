# harmonia-mattermost

## Purpose

Mattermost frontend for sending messages to Mattermost channels via the REST API. Enables the agent to communicate through self-hosted Mattermost instances.

## Channel Format

- Channel name: `mattermost`
- Sub-channel: `mattermost:<channel_id>` (Mattermost channel ID)
- Security label: `authenticated` (requires bot token)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize frontend |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll (currently no inbound implementation) |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send message to channel |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_MATTERMOST_API_URL` | `http://localhost:8065/api/v4/posts` | Mattermost API endpoint |

## Vault Symbols

- `mattermost_bot_token` -- Mattermost bot access token

## Self-Improvement Notes

- Posts JSON `{"channel_id": "...", "message": "..."}` with Bearer auth.
- Currently send-focused; poll returns no inbound messages yet.
- To add file attachments: use `/api/v4/files` endpoint first, then reference in post.
