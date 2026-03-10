# harmonia-nostr

## Purpose

Nostr protocol frontend for publishing text notes (kind 1 events) to Nostr relays. Enables the agent to post to the decentralized Nostr network.

## Channel Format

- Channel name: `nostr`
- Sub-channel: `nostr:relay` (broadcast to configured relay)
- Security label: `authenticated` (requires private key)

## FFI Contract (Frontend Standard)

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_frontend_version` | `() -> *const c_char` | Version string |
| `harmonia_frontend_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_frontend_init` | `(config: *const c_char) -> i32` | Initialize frontend |
| `harmonia_frontend_poll` | `(buf: *mut c_char, buf_len: usize) -> i32` | Poll (currently no inbound implementation) |
| `harmonia_frontend_send` | `(channel: *const c_char, payload: *const c_char) -> i32` | Publish payload as text note |
| `harmonia_frontend_last_error` | `() -> *const c_char` | Last error message |
| `harmonia_frontend_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_frontend_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_NOSTR_API_URL` | `https://relay.damus.io` | Nostr relay URL |

## Vault Symbols

- `nostr_private_key` -- Nostr private key (hex or nsec format)

## Self-Improvement Notes

- Currently uses an HTTP API proxy; real Nostr uses WebSocket.
- Currently send-focused; poll returns no inbound messages yet.
- To add native Nostr: implement NIP-01 event signing and WebSocket relay communication.
- To add reading: subscribe to events from followed pubkeys.
- To add DMs: implement NIP-04 encrypted direct messages.
