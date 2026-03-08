# harmonia-search-brave

## Purpose

Web search via Brave Search API. Sends search queries and returns structured JSON results. Provides an alternative search backend to Exa with different result characteristics.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_search_brave_version` | `() -> *const c_char` | Version string |
| `harmonia_search_brave_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_search_brave_query` | `(query: *const c_char) -> *mut c_char` | Execute search, returns JSON |
| `harmonia_search_brave_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_search_brave_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_BRAVE_API_URL` | `https://api.search.brave.com/res/v1/web/search` | API endpoint override |

## Vault Symbols

- `brave_api_key` -- Brave Search API subscription token

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_search_brave_query"
  :string "latest rust release notes" :pointer)
```

## Self-Improvement Notes

- Uses GET request with `X-Subscription-Token` header and URL-encoded query parameter.
- Returns raw Brave API JSON; Lisp side parses it.
- Brave results tend to be more web-crawl focused vs Exa's semantic search.
- To add result count: append `&count=N` to the query URL.
- To add freshness filter: append `&freshness=pd` (past day), `pw`, `pm`, `py`.
