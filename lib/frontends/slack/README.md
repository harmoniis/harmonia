# harmonia-slack

## Purpose

Slack Web API frontend. Connects to Slack workspaces to receive and send messages in configured channels using bot and app tokens.

## Channel Format

- Channel name: `slack`
- Sub-channel: `slack:<channel_id>` (Slack channel ID)
- Security label: `authenticated` (requires bot token + app token)

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

Config S-expression + vault/config-store:
| Source | Key | Description |
|--------|-----|-------------|
| Vault | `slack-bot-token` | Slack bot token (required) |
| Vault | `slack-app-token` | Slack app token (required) |
| Config-store | `slack-frontend/channels` | Comma-separated channel IDs |
| Config | `(channels ...)` | Optional channel list override |
| Config | `(bot-token ...)` | Optional bootstrap value written into vault as `slack-bot-token` |
| Config | `(app-token ...)` | Optional bootstrap value written into vault as `slack-app-token` |

Legacy env aliases for channels are resolved through config-store (`HARMONIA_SLACK_CHANNELS`).

## Self-Improvement Notes

- Client module (`client.rs`) handles Slack Web API.
- Uses polling via Slack Web API.
- Sends via `chat.postMessage` API with channel and text.
- Both bot-token and at least one channel are required to initialize.
- To add threads: include `thread_ts` in send payload for threaded replies.
- To add reactions: implement `reactions.add` API call.
