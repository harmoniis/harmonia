# Gateway And Frontends

Gateway is Harmonia's signal baseband processor.

Lisp side wrapper: `src/ports/baseband.lisp`.
Rust core: `lib/core/gateway`.

## Gateway Responsibilities

- register/unregister frontend plugins,
- parse and store frontend capabilities from config at registration time,
- adapt frontend poll output into typed **Baseband Channel Protocol** envelopes,
- send outbound payloads (with A2UI text fallback for non-capable frontends),
- expose frontend status, capabilities, and channel inventory,
- provide one unified messaging boundary to the conductor.

## Frontend Contract

Most frontends implement the standard `harmonia_frontend_*` C-ABI contract:

- version
- healthcheck
- init
- poll
- send
- last_error
- shutdown
- free_string

Frontends are hot-loaded via gateway from `config/baseband.sexp`.

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

- **2-field** (backward compatible): `sub_channel\tpayload`
- **3-field** (with metadata): `sub_channel\tpayload\tmetadata_sexp`
- **1-field** (no sub-channel): `payload`

Non-metadata frontends continue using 2-field format and are unaffected.

## Auto-Load Policy

`register-configured-frontends` supports three modes:

- `t`: always load,
- `nil`: never load,
- `:if-vault-keys`: load only if required vault symbols exist.

This keeps channel availability policy-driven and secret-aware.

## Inbound Signal Adaptation

During `:gateway-poll`:

1. Gateway polls each registered frontend.
2. Raw poll output is adapted into Baseband Channel Protocol envelopes with typed peer/body/security context.
3. `loop.lisp` converts each envelope into a typed `harmonia-signal` struct.
4. The conductor renders a clean LLM summary from that struct without re-parsing transport strings.
5. Signals with high dissonance are attenuated in security-aware routing. The conductor's policy gate blocks tainted signals from triggering privileged operations regardless of dissonance score.

When a signal carries A2UI capability, the conductor injects the A2UI component catalog into the LLM prompt context.

## A2UI Component Catalog

`config/a2ui-catalog.sexp` defines all 21 available A2UI template components with their data field specs. The conductor lazily loads and caches this catalog, injecting component names into the LLM context for A2UI-capable signals.

Text fallback: when the conductor sends an A2UI payload to a non-A2UI frontend, it extracts plain text from the component data.

## Push Integration

`lib/frontends/push` is an `rlib` (not cdylib) — a utility library consumed by mqtt-client for offline device push notifications via HTTP webhook. Not a standalone frontend.

## Outbound Flow

Conductor appends outbound messages to `*gateway-outbound-queue*`.
The loop flushes this queue in `:gateway-flush`, which keeps side effects deterministic per tick.

When sending via `gateway-send`, the conductor checks the target frontend's capabilities. A2UI payloads sent to text-only frontends are automatically degraded to plain text.

## Signal Security

Gateway signals undergo injection scanning at parse time:

1. **Dissonance scoring**: The gateway baseband scans signal payloads for injection patterns (social engineering, tool injection, Lisp reader macros) and assigns a `dissonance` score (0.0-1.0).
2. **Typed dispatch**: Loop.lisp creates nested `harmonia-signal` structs from typed envelopes with `:taint :external`. The conductor dispatches these through `orchestrate-signal`, which boundary-wraps the payload and binds `*current-originating-signal*`.
3. **Policy gate**: LLM-proposed tool commands from external signals must pass `%policy-gate`. Privileged operations are denied for tainted origins.
4. **MQTT fingerprint validation**: MQTT frontend validates `agent_fp` against vault-stored expected fingerprint. Mismatched fingerprints are downgraded to untrusted.
5. **Tailnet HMAC**: Mesh messages are authenticated with HMAC-SHA256 and protected against replay (5-minute window).
