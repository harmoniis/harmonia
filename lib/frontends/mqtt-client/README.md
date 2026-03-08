# harmonia-mqtt-client

## Purpose

MQTT broker channel for pub/sub messaging. Connects to an MQTT broker, publishes messages to topics, polls for incoming messages, and supports TLS with client certificates. Also implements the standard frontend FFI contract for gateway integration.

## Channel Format

- Channel name: `mqtt`
- Sub-channel: `mqtt:<topic>` (MQTT topic path)
- Security label: `network` (requires broker access, optional TLS)

## FFI Surface -- MQTT-Specific

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_mqtt_client_version` | `() -> *const c_char` | Version string |
| `harmonia_mqtt_client_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_mqtt_client_publish` | `(topic: *const c_char, payload: *const c_char) -> i32` | Publish to topic |
| `harmonia_mqtt_client_poll` | `(topic: *const c_char) -> *mut c_char` | Poll topic for messages |
| `harmonia_mqtt_client_reset` | `() -> i32` | Reset connection state |
| `harmonia_mqtt_client_make_envelope` | `(from: *const c_char, channel: *const c_char, body: *const c_char, msg_type: *const c_char) -> *mut c_char` | Create structured envelope |
| `harmonia_mqtt_client_parse_envelope` | `(payload: *const c_char) -> *mut c_char` | Parse structured envelope |
| `harmonia_mqtt_client_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_mqtt_client_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## FFI Surface -- Frontend Contract

Also exports `harmonia_frontend_*` (init, poll, send, shutdown, etc.) for gateway registration.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_MQTT_BROKER` | `test.mosquitto.org:1883` | Broker host:port |
| `HARMONIA_MQTT_TIMEOUT_MS` | `5000` | Connection timeout |
| `HARMONIA_MQTT_TLS` | `0` | Enable TLS (`1` to enable) |
| `HARMONIA_MQTT_CA_CERT` | -- | CA certificate path (required if TLS=1) |
| `HARMONIA_MQTT_CLIENT_CERT` | -- | Client certificate path (mTLS) |
| `HARMONIA_MQTT_CLIENT_KEY` | -- | Client key path (mTLS) |

## Self-Improvement Notes

- Uses raw TCP sockets with manual MQTT CONNECT/PUBLISH/SUBSCRIBE packet construction.
- TLS support via `rustls` with optional mutual TLS (client cert + key).
- Envelope format is S-expression: `(:from "..." :channel "..." :body "..." :type "..." :ts ...)`.
- The frontend contract bridges MQTT into gateway's unified signal bus.
- To add QoS levels: extend publish with QoS parameter (currently QoS 0).
- To add wildcard subscriptions: implement SUBSCRIBE with `#` and `+` topic filters.
