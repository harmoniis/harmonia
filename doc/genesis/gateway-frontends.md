# Gateway And Frontends

Gateway is Harmonia's signal baseband processor.

Lisp side wrapper: `src/ports/baseband.lisp`.
Rust core: `lib/core/gateway`.

## Gateway Responsibilities

- intercept and dispatch ALL /commands from ALL frontends (unified command dispatch),
- register/unregister frontend plugins,
- parse and store frontend capabilities from config at registration time,
- adapt frontend poll output into typed **Baseband Channel Protocol** envelopes,
- send outbound payloads (with A2UI text fallback for non-capable frontends),
- expose frontend status, capabilities, and channel inventory,
- provide one unified messaging boundary to the conductor.

## Unified Command Dispatch

The gateway is the single interception point for ALL /commands from ALL frontends (TUI, MQTT, Telegram, Tailscale, paired nodes).

Source: `lib/core/gateway/src/command_dispatch.rs`.

Commands are handled in two tiers:

1. **Native** — fully executed in Rust: `/wallet`, `/identity`, `/help`.
2. **Delegated** — routed to a Lisp-registered callback for runtime state access: `/status`, `/backends`, `/frontends`, `/tools`, `/chronicle`, `/metrics`, `/security`, `/feedback`, `/exit`.

Security enforcement happens at the gateway before dispatch:
- Read-restricted commands require Owner or Authenticated security label.
- `/exit` is TUI-only.

The Lisp callback for delegated commands is registered during `init-baseband-port`. The gateway calls it for delegated commands via IPC.

For `/exit`: the gateway sets a `pending_exit` flag. Lisp checks this after each poll and stops the run-loop.

Lisp never sees command envelopes — only agent-level prompts pass through to the orchestrator.

## Frontend Contract

Frontends are rlib crates compiled directly into `harmonia-runtime`. They are no longer separate shared libraries loaded via dlopen.

Each frontend implements a standard trait contract providing:

- version
- healthcheck
- init
- poll
- send
- last_error
- shutdown

Frontend configuration is declared in `config/baseband.sexp`.

## Frontend Capabilities

Each frontend declares capabilities in `config/baseband.sexp` via `:capabilities`:

```lisp
(:name "mqtt"
 :capabilities (:a2ui "1.0" :push "t")
 ...)
```

At registration, the gateway parses these into `Capability` structs stored in `FrontendHandle`. Capabilities are:

- **Static per-frontend** — declared in config, not changeable at runtime.
- **Attached to every channel envelope** from that frontend as typed capability metadata.
- **Queryable** via `gateway-frontend-status` (includes capabilities in output).

This makes A2UI dispatch generic: any frontend declaring `:a2ui` gets A2UI treatment. No hardcoded frontend-name checks.

## Baseband Channel Envelope

A gateway envelope carries:

- `id`, `version`, `kind`, `type-name`
- `channel` — semantic channel kind + address + label
- `peer` — peer identity and device/fingerprint metadata
- `body` — normalized text/raw payload
- `capabilities` — per-channel capability map
- `security` — typed trust label and fingerprint validity
- `audit` — timestamp + dissonance
- `transport` — gateway-private transport context carried for diagnostics

S-expr output:
```lisp
(:id 42 :version 1 :kind "external" :type-name "message.text"
 :channel (:kind "mqtt" :address "topic/foo" :label "mqtt:topic/foo")
 :peer (:id "uuid-123" :platform "ios" :device-id "uuid-123")
 :body (:format "text" :text "hello" :raw "hello")
 :capabilities (:a2ui "1.0" :push "t")
 :security (:label "authenticated" :source "mqtt-envelope" :fingerprint-valid t)
 :audit (:timestamp-ms 1709712000000 :dissonance 0.0)
 :attachments nil
 :transport (:kind "mqtt" :raw-address "topic/foo"))
```

## Poll Format

Frontends emit newline-separated lines in one of these formats:

- **3-field**: `sub_channel\tpayload\tmetadata_sexp` — standard format with origin metadata.
- **2-field**: `sub_channel\tpayload` — gateway fills defaults from `default_channel_class` and `default_node_role`.
- **1-field**: `payload` — no sub-channel; gateway infers from frontend name.

Metadata is an S-expression with signal origin context:
```
(:channel-class "telegram-bot" :node-id "12345" :remote t)
```

All production frontends emit 3-field triples. The gateway's `default_channel_class` and `default_node_role` provide safety-net defaults for every frontend type.

## Auto-Load Policy

`register-configured-frontends` supports four modes:

- `t`: always load,
- `nil`: never load,
- `:if-vault-keys`: load only if required vault symbols exist,
- `:if-ready`: load only if vault keys exist AND the frontend library file is present on disk.

Frontends can declare `:platforms` to restrict loading to specific operating systems:

```lisp
(:name "imessage"
 :auto-load :if-ready
 :platforms (:macos)
 ...)
```

When `:platforms` is set, the frontend is skipped on non-matching platforms regardless of auto-load mode.

## Inbound Signal Adaptation

During `:gateway-poll`:

1. Gateway polls each registered frontend.
2. Sender policy filter (`sender_policy.rs`) applies: signals from messaging frontends (email, Slack, Discord, Signal, WhatsApp, iMessage, Telegram, Mattermost, Nostr) are dropped unless the sender is in the frontend's allowlist or the frontend is in allow-all mode. TUI, MQTT, and Tailscale are exempt.
3. Command envelopes (`/wallet`, `/status`, etc.) are intercepted by `command_dispatch` — handled in Rust or delegated to Lisp callback — and filtered out. Responses are sent back to the originating frontend.
4. Remaining envelopes (agent prompts) are adapted into Baseband Channel Protocol envelopes with typed peer/body/security context.
4. `loop.lisp` converts each envelope into a typed `harmonia-signal` struct.
5. The conductor renders a clean LLM summary from that struct without re-parsing transport strings.
6. Signals with high dissonance are attenuated in security-aware routing. The conductor's policy gate blocks tainted signals from triggering privileged operations regardless of dissonance score.

When a signal carries A2UI capability, the conductor injects the A2UI component catalog into the LLM prompt context.

## A2UI Component Catalog

`config/a2ui-catalog.sexp` defines all 21 available A2UI template components with their data field specs. The conductor lazily loads and caches this catalog, injecting component names into the LLM context for A2UI-capable signals.

Text fallback: when the conductor sends an A2UI payload to a non-A2UI frontend, it extracts plain text from the component data.

## Push Integration

`lib/frontends/push` is a utility library consumed by mqtt-client for offline device push notifications via HTTP webhook. Not a standalone frontend.

## Outbound Flow

Conductor appends outbound messages to `*gateway-outbound-queue*`.
The loop flushes this queue in `:gateway-flush`, which keeps side effects deterministic per tick.

When sending via `gateway-send`, the conductor checks the target frontend's capabilities. A2UI payloads sent to text-only frontends are automatically degraded to plain text.

## Signal Security

Gateway signals undergo sender filtering and injection scanning at parse time:

1. **Sender policy**: Messaging frontends default to deny-all. Signals from unknown senders are dropped at the gateway before any further processing. Allowlists are managed via `/policies` TUI command or config-store (`sender-policy` scope). TUI, MQTT, and Tailscale frontends are exempt.
2. **Dissonance scoring**: The gateway baseband scans signal payloads for injection patterns (social engineering, tool injection, Lisp reader macros) and assigns a `dissonance` score (0.0-1.0).
3. **Typed dispatch**: Loop.lisp creates nested `harmonia-signal` structs from typed envelopes with `:taint :external`. The conductor dispatches these through `orchestrate-signal`, which boundary-wraps the payload and binds `*current-originating-signal*`.
4. **Policy gate**: LLM-proposed tool commands from external signals must pass `%policy-gate`. Privileged operations are denied for tainted origins.
5. **MQTT fingerprint validation**: MQTT frontend validates `agent_fp` against vault-stored expected fingerprint. Mismatched fingerprints are downgraded to untrusted.
6. **Tailnet HMAC**: Mesh messages are authenticated with HMAC-SHA256 and protected against replay (5-minute window).
