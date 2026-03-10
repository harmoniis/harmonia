# harmonia-telegram

## Purpose

Telegram Bot API frontend. Connects to the Telegram Bot API to receive and send messages, enabling the agent to operate as a Telegram bot.

## Channel Format

- Channel name: `telegram`
- Sub-channel: `telegram:<chat_id>` (Telegram chat ID)
- Security label: `authenticated` (requires bot token)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for new messages (long-polling) |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send message to chat_id |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Config S-expression + vault:
| Source | Key | Description |
|--------|-----|-------------|
| Vault | `telegram-bot-token` | Telegram bot token (required) |
| Config | `:bot-token` | Optional bootstrap value written into vault as `telegram-bot-token` |

No direct secret env fallback is used by the frontend runtime.

## Self-Improvement Notes

- Bot module (`bot.rs`) handles Telegram Bot API via HTTP.
- Uses `getUpdates` long-polling for incoming messages.
- Sends via `sendMessage` API with `chat_id` and `text`.
- To add inline keyboards: extend send payload with `reply_markup` JSON.
- To add media: use `sendPhoto`, `sendDocument` API methods.
- To add webhook mode: implement an HTTP listener instead of polling.
