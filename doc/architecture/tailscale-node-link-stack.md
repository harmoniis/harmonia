# Tailscale Node-Link Stack

## Chosen stack

- Noise protocol engine: `snow 0.10.0`
- Tailscale daemon integration: `tailscale-localapi 0.5.0`
- Embedded tailscale fallback: official `libtailscale` via FFI if userspace mode is required later

## Why

`snow` is the correct Rust-side choice for Harmonia's node-link protocol because it is a current, focused Noise implementation with explicit support for the exact handshake builder flow we need:

- `Noise_XX_25519_ChaChaPoly_BLAKE2s` for first pairing
- `Noise_IK_25519_ChaChaPoly_BLAKE2s` for resumed sessions after both peers know each other's static keys

Tailscale should remain the network substrate, not be rebuilt from raw WireGuard crates. Harmonia needs:

- tailnet identity and peer status
- cross-platform access to the local `tailscaled`
- a clean path for client-only nodes and full agent nodes

`tailscale-localapi` is the best current Rust fit for the installed-daemon path. It supports:

- Unix socket access on Linux and other Unix systems
- local TCP plus same-user password on macOS and Windows
- `status`, `whois`, and certificate retrieval

## Explicit non-choice

Do not build Harmonia's tailscale layer from `boringtun`, `wireguard-control`, or similar crates. Those are useful lower-level components, but they do not replace Tailscale's control plane, identity model, peer discovery, or LocalAPI.

## Current implementation boundary

The current CLI node-link layer now persists a Noise static identity per node, advertises the selected protocol stack in pairing invites, and resolves remote peers through the local tailscaled status when available.

Node RPC (`lib/core/node-rpc`) supports remote frontend pairing via Tailscale mesh. Active capabilities:

- Noise-authenticated invite accept
- persisted trust/grants
- node-to-node RPC for remote frontend pairing and command dispatch
- wallet-derived key material
