# harmonia-http

## Purpose

HTTP client with vault-integrated authentication. Provides simple GET/POST with optional Bearer token injection from vault symbols, so no secrets appear in calling code.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_http_version` | `() -> *const c_char` | Version string |
| `harmonia_http_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_http_request` | `(method: *const c_char, url: *const c_char) -> *mut c_char` | Unauthenticated HTTP request |
| `harmonia_http_request_with_auth_symbol` | `(method: *const c_char, url: *const c_char, auth_symbol: *const c_char) -> *mut c_char` | HTTP request with vault Bearer auth |
| `harmonia_http_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_http_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

No special env vars. Uses `curl` CLI under the hood. Vault must be initialized for authenticated requests.

## Usage from Lisp

```lisp
;; Simple GET
(let ((body (cffi:foreign-funcall "harmonia_http_request"
              :string "GET" :string "https://api.example.com/status" :pointer)))
  (cffi:foreign-string-to-lisp body))

;; Authenticated POST
(cffi:foreign-funcall "harmonia_http_request_with_auth_symbol"
  :string "POST" :string "https://api.example.com/data"
  :string "my_api_key" :pointer)
```

## Self-Improvement Notes

- Implemented via `curl -sS` subprocess; no async runtime needed.
- `request_with_auth_symbol` calls `get_secret_for_component("http-backend", auth_symbol)` and injects `Authorization: Bearer <secret>`.
- `http-backend` is deny-by-default unless explicitly allowed via `HARMONIA_VAULT_COMPONENT_POLICY` (example: `http-backend=my-api-key,*service-token`).
- To add request body support: add a `body` parameter to the FFI.
- To add custom headers: accept a headers string (S-expression or JSON).
- To replace curl with native Rust: use `ureq` or `reqwest` (already in some deps).
