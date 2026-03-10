# harmonia-email-client

## Purpose

Email sending frontend via HTTP API (e.g., SendGrid, Mailgun, or compatible transactional email service). Sends emails with configurable sender address.

## Channel Format

- Channel name: `email`
- Sub-channel: `email:<recipient_address>` (target email)
- Security label: `authenticated` (requires API key)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize frontend |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll (currently no inbound implementation) |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send email to recipient (subject via env default) |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_EMAIL_API_URL` | `https://api.sendgrid.com/v3/mail/send` | Email API endpoint |
| `HARMONIA_EMAIL_FROM` | `harmonia@local.invalid` | Sender address |
| `HARMONIA_EMAIL_DEFAULT_SUBJECT` | `Harmonia message` | Default subject used by gateway send wrapper |

## Vault Symbols

- `email_api_key` -- Email service API key (Bearer token)

## Self-Improvement Notes

- Sends JSON payload via POST with Bearer auth from vault.
- Currently send-focused; poll returns no inbound messages yet.
- To add receive/poll: implement IMAP or webhook-based inbox monitoring.
- To add HTML emails: extend payload with `content_type` field.
- To add attachments: base64 encode and include in the API payload.
