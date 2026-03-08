# harmonia-search-exa

## Purpose

Web search via Exa.ai API. Sends search queries and returns structured JSON results with titles, URLs, and snippets. Supports neural and keyword search modes.

## FFI Surface

| Export | Signature | Description |
|--------|-----------|-------------|
| `harmonia_search_exa_version` | `() -> *const c_char` | Version string |
| `harmonia_search_exa_healthcheck` | `() -> i32` | Returns 1 if alive |
| `harmonia_search_exa_query` | `(query: *const c_char) -> *mut c_char` | Execute search, returns JSON |
| `harmonia_search_exa_last_error` | `() -> *mut c_char` | Last error message |
| `harmonia_search_exa_free_string` | `(ptr: *mut c_char)` | Free returned strings |

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `HARMONIA_EXA_API_URL` | `https://api.exa.ai/search` | API endpoint override |

## Vault Symbols

- `exa_api_key` -- Exa.ai API key

## Usage from Lisp

```lisp
(cffi:foreign-funcall "harmonia_search_exa_query"
  :string "rust async runtime comparison" :pointer)
```

## Self-Improvement Notes

- Posts JSON `{"query": "...", "numResults": 5}` to Exa API via curl.
- Returns raw JSON response; Lisp side parses it.
- To add num_results parameter: extend FFI with `harmonia_search_exa_query_n(query, n)`.
- To add content retrieval: use Exa's `contents` endpoint for full-text extraction.
- To add search type selection: add a `type` parameter (keyword/neural/auto).
