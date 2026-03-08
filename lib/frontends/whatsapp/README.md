# harmonia-whatsapp

## Purpose

WhatsApp frontend via a bridge API (e.g., WhatsApp Business API or compatible gateway). Polls for incoming messages and sends replies.

## Channel Format

- Channel name: `whatsapp`
- Sub-channel: `whatsapp:<phone_number>` (recipient phone number)
- Security label: `authenticated` (requires API key)

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
| Config | `:api-url` | WhatsApp bridge API URL |
| Env | `HARMONIA_WHATSAPP_API_URL` | Fallback API URL |
| Config | `:api-key` | API authentication key |
| Env | `HARMONIA_WHATSAPP_API_KEY` | Fallback API key |

## Self-Improvement Notes

- Client module (`client.rs`) handles HTTP API integration.
- Uses long-polling or webhook model depending on the bridge.
- To add media messaging: extend send with media_url parameter.
- To add read receipts: implement via bridge API status endpoints.
