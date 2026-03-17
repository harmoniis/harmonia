# harmonia-gateway

## Purpose

Signal baseband processor that hot-loads frontend `.so` plugins at runtime.
Gateway is the central message bus: frontends register, messages are polled/dispatched through a unified FFI contract.

## FFI Surface

### Frontend Management

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_gateway_version` | `() -> *const c_char` | Version string |
| `harmonia_gateway_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_gateway_init` | `() -> i32` | Initialize gateway state |
| `harmonia_gateway_register` | `(name, so_path, config_sexp, security_label: *const c_char) -> i32` | Register a frontend .so |
| `harmonia_gateway_unregister` | `(name: *const c_char) -> i32` | Unregister a frontend |
| `harmonia_gateway_reload` | `(name: *const c_char) -> i32` | Hot-reload a frontend |
| `harmonia_gateway_crash_count` | `(name: *const c_char) -> i32` | Crash count for a frontend |
| `harmonia_gateway_poll` | `() -> *mut c_char` | Poll all frontends for inbound signals |
| `harmonia_gateway_send` | `(frontend, channel, payload: *const c_char) -> i32` | Send outbound signal |
| `harmonia_gateway_list_frontends` | `() -> *mut c_char` | List registered frontends (S-expression) |
| `harmonia_gateway_frontend_status` | `(name: *const c_char) -> *mut c_char` | Status of a specific frontend |
| `harmonia_gateway_list_channels` | `(name: *const c_char) -> *mut c_char` | List channels for a frontend |

### Tool Management

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_gateway_register_tool` | `(name, so_path, config_sexp, security_label: *const c_char) -> i32` | Register a tool .so |
| `harmonia_gateway_unregister_tool` | `(name: *const c_char) -> i32` | Unregister a tool |
| `harmonia_gateway_invoke_tool` | `(name, operation, input: *const c_char) -> *mut c_char` | Invoke a tool operation |
| `harmonia_gateway_list_tools` | `() -> *mut c_char` | List registered tools |
| `harmonia_gateway_tool_capabilities` | `(name: *const c_char) -> *mut c_char` | Tool capabilities |
| `harmonia_gateway_tool_status` | `(name: *const c_char) -> *mut c_char` | Tool status |
| `harmonia_gateway_reload_tool` | `(name: *const c_char) -> i32` | Hot-reload a tool |

### Command Dispatch & Lifecycle

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_gateway_set_command_query` | `(callback: extern fn(*const c_char) -> *mut c_char) -> i32` | Register Lisp command callback |
| `harmonia_gateway_pending_exit` | `() -> i32` | Check if `/exit` was requested |
| `harmonia_gateway_shutdown` | `() -> i32` | Graceful shutdown |
| `harmonia_gateway_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_gateway_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_STATE_ROOT` | `$TMPDIR/harmonia` | Base state directory |

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_gateway_init" :int)
(cffi:foreign-funcall "harmonia_gateway_register"
  :string "telegram"
  :string "/path/to/libharmonia_telegram.so"
  :string "(:bot-token \"...\")"
  :string "authenticated"
  :int)
(let ((msg (cffi:foreign-funcall "harmonia_gateway_poll" :pointer)))
  (when msg (process-signal (cffi:foreign-string-to-lisp msg))))
```

## Self-Improvement Notes

- Gateway uses `libloading` to dlopen frontend .so files; each frontend implements the `harmonia_frontend_*` FFI contract.
- The poll loop is non-blocking; Lisp drives the event loop cadence.
- To add a new frontend: implement the 8-function frontend FFI contract (`version`, `healthcheck`, `init`, `poll`, `send`, `last_error`, `shutdown`, `free_string`), compile to cdylib, register via gateway.
- Hot-reload: unregister + register with new .so path. No process restart needed.
- State is held in a `RwLock<HashMap>` keyed by frontend name.
- Sessionful transports such as MQTT, Tailscale, and HTTP/2 mTLS keep transport identity in `sub-channel`. HTTP/2 uses `<identity-fingerprint>/<session-id>/<channel>`, which preserves multi-session parallelism without changing the gateway ABI.
