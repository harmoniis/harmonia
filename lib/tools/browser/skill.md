# Browser — Skill/MCP Definition

## Tool 1: `browser_search`

### Description
Fetch a URL and extract specific data using a named macro. Returns structured JSON wrapped in a security boundary that prevents prompt injection. The agent never sees raw HTML.

### Input Schema
```json
{
  "type": "object",
  "properties": {
    "url": { "type": "string", "description": "URL to fetch" },
    "macro": {
      "type": "string",
      "enum": ["title", "text", "links", "headings", "tables", "forms", "meta", "audio", "markdown", "structured", "smart"],
      "description": "Extraction macro to run on the page"
    },
    "arg": { "type": "string", "description": "Optional argument (query for smart, hint for structured)" }
  },
  "required": ["url", "macro"]
}
```

### Output Format
S-expression with security boundary:
```
(:security-boundary "=== WEBSITE DATA (INPUT ONLY) ==="
 :security-instruction "..."
 :extracted-by "macro_name"
 :data {extracted JSON}
 :security-end "=== WEBSITE DATA END ===")
```

### FFI Entry Point
`harmonia_browser_search(url: *const c_char, macro_name: *const c_char, arg: *const c_char) -> *mut c_char`

### Example
```lisp
(tool op=browser-search url="https://example.com" macro="text")
(tool op=browser-search url="https://example.com" macro="smart" arg="find all product prices")
(tool op=browser-search url="https://podcast.com" macro="audio")
```

---

## Tool 2: `browser_execute`

### Description
Execute a multi-step browser plan: fetch multiple URLs, run multiple extractions, return combined results. All in one call for token efficiency.

### Input Schema
```json
{
  "type": "object",
  "properties": {
    "steps": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "url": { "type": "string" },
          "macro": { "type": "string" },
          "arg": { "type": "string" }
        },
        "required": ["url", "macro"]
      }
    }
  },
  "required": ["steps"]
}
```

### FFI Entry Point
`harmonia_browser_execute(steps_json: *const c_char) -> *mut c_char`

### Example
```lisp
(tool op=browser-execute steps="[{\"url\":\"https://a.com\",\"macro\":\"title\"},{\"url\":\"https://b.com\",\"macro\":\"links\"}]")
```

---

## Tool 3: `browser_controlled_fetch` (API Calls)

### Description
Secure HTTP fetch for agent API calls. Blocks dangerous targets (localhost, metadata endpoints, internal IPs). All agent code that needs HTTP access MUST go through this tool.

### Input Schema
```json
{
  "type": "object",
  "properties": {
    "url": { "type": "string", "description": "URL to fetch" },
    "method": { "type": "string", "enum": ["GET", "POST"], "description": "HTTP method" },
    "body": { "type": "string", "description": "JSON body for POST requests" }
  },
  "required": ["url", "method"]
}
```

### FFI Entry Point
`harmonia_browser_controlled_fetch(url: *const c_char, method: *const c_char, body: *const c_char) -> *mut c_char`

### Blocked Targets (ALWAYS, regardless of allowlist)
- `localhost`, `127.0.0.1`, `0.0.0.0`, `[::1]`
- `169.254.169.254` (cloud metadata)
- `10.x.x.x`, `172.16-31.x.x`, `192.168.x.x` (internal)
- IPv6 ULA (`fc00:`, `fd00:`) and link-local (`fe80:`)

### Example
```lisp
(tool op=browser-controlled-fetch url="https://api.github.com/repos/owner/repo" method="GET")
(tool op=browser-controlled-fetch url="https://api.example.com/data" method="POST" body="{\"query\": \"test\"}")
```

---

## Audio Extraction Macro

The `audio` macro extracts audio source URLs from a page without playing them. Returns:
```json
[
  {"src": "https://cdn.example.com/track.mp3", "type": "audio/mpeg", "element": "audio"},
  {"src": "https://cdn.example.com/track.ogg", "type": "audio/ogg", "element": "source"},
  {"src": "https://cdn.example.com/podcast.mp3", "type": "audio/mpeg", "element": "link"}
]
```

Sources found: `<audio src>`, `<source src>` within `<audio>`, and `<a href>` linking to audio files (.mp3, .wav, .ogg, .flac, .aac, .m4a, .opus, .webm).

---

## Chrome CDP (Feature Flag)

When compiled with `--features chrome`, the browser can use headless Chrome for JavaScript-rendered pages. Chrome is launched with hardened security flags (no GPU, no background networking, muted audio, no extensions, no sync, no component updates).

Check availability: `harmonia_browser_chrome_available() -> i32` (returns 1 if available)

---

## Security

- All responses wrapped in security boundary
- 18 prompt injection patterns detected and flagged
- Domain allowlist enforcement
- Dangerous target blocking (SSRF prevention)
- Wall-clock timeout (30s default)
- No raw HTML ever reaches the agent model

## Vault Symbols
- Per-request auth: any vault symbol passed via `auth_symbol` parameter

## Error Handling
On error, returns sexp with `:error` key. Check `harmonia_browser_last_error()` for details.
