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

Config S-expression or env vars:
| Source | Key | Description |
|--------|-----|-------------|
| Config | `(bot-token ...)` | Slack bot token (xoxb-...) |
| Env | `HARMONIA_SLACK_BOT_TOKEN` | Fallback bot token |
| Config | `(app-token ...)` | Slack app token (xapp-...) |
| Env | `HARMONIA_SLACK_APP_TOKEN` | Fallback app token |
| Config | `(channels ...)` | Channel IDs to monitor |
| Env | `HARMONIA_SLACK_CHANNELS` | Comma-separated channel IDs |

## Self-Improvement Notes

- Client module (`client.rs`) handles Slack Web API.
- Uses Socket Mode (app token) for real-time events or falls back to polling.
- Sends via `chat.postMessage` API with channel and text.
- Both bot-token and at least one channel are required to initialize.
- To add threads: include `thread_ts` in send payload for threaded replies.
- To add reactions: implement `reactions.add` API call.
