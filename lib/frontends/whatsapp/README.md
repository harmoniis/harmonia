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

Config S-expression + vault/config-store:
| Source | Key | Description |
|--------|-----|-------------|
| Config-store | `whatsapp-frontend/api-url` | WhatsApp bridge API URL |
| Vault | `whatsapp-session` | WhatsApp bridge authentication token |
| Config | `:api-url` | Optional bootstrap value written into config-store |
| Config | `:api-key` | Optional bootstrap value written into vault as `whatsapp-session` |

Legacy env alias for `api-url` is resolved through config-store (`HARMONIA_WHATSAPP_API_URL`).
Legacy vault bridge URL symbol (`whatsapp-bridge-url`) is migrated to config-store on init.

## Self-Improvement Notes

- Client module (`client.rs`) handles HTTP API integration.
- Uses long-polling or webhook model depending on the bridge.
- To add media messaging: extend send with media_url parameter.
- To add read receipts: implement via bridge API status endpoints.
