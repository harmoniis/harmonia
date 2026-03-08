# harmonia-mattermost

## Purpose

Mattermost frontend for sending messages to Mattermost channels via the REST API. Enables the agent to communicate through self-hosted Mattermost instances.

## Channel Format

- Channel name: `mattermost`
- Sub-channel: `mattermost:<channel_id>` (Mattermost channel ID)
- Security label: `authenticated` (requires bot token)

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_mattermost_version` | `() -> *const c_char` | Version string |
| `harmonia_mattermost_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_mattermost_send_text` | `(channel_id: *const c_char, text: *const c_char) -> i32` | Send message to channel |
| `harmonia_mattermost_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_mattermost_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_MATTERMOST_API_URL` | `http://localhost:8065/api/v4/posts` | Mattermost API endpoint |

## Vault Symbols

- `mattermost_bot_token` -- Mattermost bot access token

## Self-Improvement Notes

- Posts JSON `{"channel_id": "...", "message": "..."}` with Bearer auth.
- Currently send-only; to add receive: implement websocket connection for events.
- To add file attachments: use `/api/v4/files` endpoint first, then reference in post.
