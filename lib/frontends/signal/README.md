# harmonia-signal

## Purpose

Signal frontend bridge for Harmonia using a `signal-cli` compatible HTTP API (REST/JSON-RPC gateway). Supports inbound polling and outbound messaging while keeping Signal protocol complexity outside the agent process.

## Channel Format

- Channel name: `signal`
- Sub-channel: `signal:<phone_or_group>`
- Security label: `authenticated` (requires registered Signal account)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize with config |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll for inbound Signal messages |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Send Signal message to recipient/group |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

Config S-expression + vault/config-store:
| Source | Key | Description |
|--------|-----|-------------|
| Config-store | `signal-frontend/rpc-url` | signal-cli bridge base URL (default `http://127.0.0.1:8080`) |
| Config-store | `signal-frontend/account` | Registered Signal account/number (required) |
| Vault | `signal-auth-token` | Optional bridge bearer token |
| Config | `:rpc-url`, `:account` | Optional bootstrap values written into config-store |
| Config | `:auth-token` | Optional bootstrap value written into vault as `signal-auth-token` |

Legacy env aliases for non-secret values are resolved through config-store:
`HARMONIA_SIGNAL_RPC_URL`, `HARMONIA_SIGNAL_ACCOUNT`.

Send channel conventions:
- `recipient:+15551234567` or plain `+15551234567` for direct messages
- `group:<group_id>` for group sends

## Self-Improvement Notes

- Uses endpoint fallback (`/v1` then `/v2`) to support multiple signal-cli bridge variants.
- Tracks `last_timestamp_ms` cursor to reduce duplicate inbound event replay.
- Treats Signal transport as an external channel adapter; cryptographic protocol internals remain in signal-cli/libsignal stack.
- Legacy vault non-secret keys (`signal-rpc-url`, `signal-account`) are migrated into config-store on init.
- For production hardening: pin bridge version, enforce TLS/mTLS, and run bridge behind a minimal network boundary.
