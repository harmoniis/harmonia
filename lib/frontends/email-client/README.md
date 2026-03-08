# harmonia-email-client

## Purpose

Email sending frontend via HTTP API (e.g., SendGrid, Mailgun, or compatible transactional email service). Sends emails with configurable sender address.

## Channel Format

- Channel name: `email`
- Sub-channel: `email:<recipient_address>` (target email)
- Security label: `authenticated` (requires API key)

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_email_client_version` | `() -> *const c_char` | Version string |
| `harmonia_email_client_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_email_client_send` | `(to: *const c_char, subject: *const c_char, body: *const c_char) -> i32` | Send email |
| `harmonia_email_client_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_email_client_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_EMAIL_API_URL` | `https://api.sendgrid.com/v3/mail/send` | Email API endpoint |
| `HARMONIA_EMAIL_FROM` | `harmonia@local.invalid` | Sender address |

## Vault Symbols

- `email_api_key` -- Email service API key (Bearer token)

## Self-Improvement Notes

- Sends JSON payload via POST with Bearer auth from vault.
- To add receive/poll: implement IMAP or webhook-based inbox monitoring.
- To add HTML emails: extend payload with `content_type` field.
- To add attachments: base64 encode and include in the API payload.
