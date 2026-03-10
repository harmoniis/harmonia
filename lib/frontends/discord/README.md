# harmonia-discord

## Purpose

Discord frontend using the Discord Bot API. Polls configured channels for new messages and sends outbound replies to target channels.

## Channel Format

- Channel name: `discord`
- Sub-channel: `discord:<channel_id>`
- Security label: `authenticated` (requires bot token)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for new channel messages |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send message to a channel |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Config S-expression + vault/config-store:
| Source | Key | Description |
|--------|-----|-------------|
| Vault | `discord-bot-token` | Discord bot token (required) |
| Config-store | `discord-frontend/channels` | Comma-separated channel IDs |
| Config | `(channels ...)` | Optional channel list override |
| Config | `(bot-token ...)` | Optional bootstrap value written into vault as `discord-bot-token` |

Legacy env aliases for channels are resolved through config-store (`HARMONIA_DISCORD_CHANNELS`).

## Self-Improvement Notes

- Uses Discord REST API (`/channels/{id}/messages`) for inbound polling and outbound send.
- First poll advances an internal cursor per channel to avoid replaying old history.
- Skips bot-authored messages and empty payloads.
- For command routing, map slash commands through a dedicated bot interaction endpoint in a future revision.
